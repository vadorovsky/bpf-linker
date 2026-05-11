use std::{marker::PhantomData, ptr::NonNull};

use llvm_sys::{
    LLVMValue,
    core::{
        LLVMCountParams, LLVMDisposeValueMetadataEntries, LLVMGetFirstBasicBlock,
        LLVMGetNextBasicBlock, LLVMGetNumOperands, LLVMGetOperand, LLVMGetParam,
        LLVMGlobalCopyAllMetadata, LLVMIsAFunction, LLVMIsAGlobalObject, LLVMIsAInstruction,
        LLVMIsAMDNode, LLVMIsAUser, LLVMMDNodeInContext2, LLVMMDStringInContext2,
        LLVMMetadataAsValue, LLVMPrintValueToString, LLVMReplaceMDNodeOperandWith,
        LLVMValueAsMetadata, LLVMValueMetadataEntriesGetKind, LLVMValueMetadataEntriesGetMetadata,
    },
    debuginfo::{LLVMGetMetadataKind, LLVMGetSubprogram, LLVMMetadataKind, LLVMSetSubprogram},
    prelude::{
        LLVMBasicBlockRef, LLVMContextRef, LLVMMetadataRef, LLVMValueMetadataEntry, LLVMValueRef,
    },
};

use crate::llvm::{
    LLVMContext, Message, symbol_name,
    types::{
        LLVMTypeError,
        di::{DICompositeType, DIDerivedType, DISubprogram, DIType},
    },
};

/// A trait for wrappers that represent LLVM `Value` subclasses.
pub(crate) trait ValueLike
where
    Self: Sized,
{
    const TYPE_NAME: &'static str;

    /// Returns whether the provided value pointer has the LLVM runtime type
    /// expected by this wrapper.
    fn check_value_type(value: NonNull<LLVMValue>) -> bool;

    /// Constructs a new [`Self`] from a non-null LLVM value without checking
    /// its runtime type.
    ///
    /// # Safety
    ///
    /// The provided value must be a valid instance of the LLVM type represented
    /// by this wrapper.
    unsafe fn from_non_null_unchecked(value: NonNull<LLVMValue>) -> Self;

    /// Constructs a new [`Self`] from a raw LLVM value after checking its
    /// runtime type.
    fn from_raw(value: LLVMValueRef) -> Result<Self, LLVMTypeError> {
        let value = NonNull::new(value).ok_or_else(|| LLVMTypeError::NullPtr(Self::TYPE_NAME))?;
        Self::from_non_null(value)
    }

    /// Constructs a new [`Self`] from a raw LLVM value without checking its
    /// runtime type.
    ///
    /// This method still rejects null pointers.
    ///
    /// # Safety
    ///
    /// If the provided value must:
    ///
    /// - Not be `NULL`.
    /// - Be a valid instance of the LLVM type represented by this wrapper.
    unsafe fn from_raw_unchecked(value: LLVMValueRef) -> Result<Self, LLVMTypeError> {
        let value = NonNull::new(value).ok_or_else(|| LLVMTypeError::NullPtr(Self::TYPE_NAME))?;
        Ok(unsafe { Self::from_non_null_unchecked(value) })
    }

    /// Constructs a new [`Self`] from a non-null LLVM value after checking its
    /// runtime type.
    fn from_non_null(value: NonNull<LLVMValue>) -> Result<Self, LLVMTypeError> {
        if Self::check_value_type(value) {
            // SAFETY: We checked that the type matches.
            Ok(unsafe { Self::from_non_null_unchecked(value) })
        } else {
            Err(LLVMTypeError::IncorrectType)
        }
    }
}

pub(crate) fn replace_name(
    value_ref: LLVMValueRef,
    context: LLVMContextRef,
    name_operand_index: u32,
    name: &[u8],
) {
    let name = unsafe { LLVMMDStringInContext2(context, name.as_ptr().cast(), name.len()) };
    unsafe { LLVMReplaceMDNodeOperandWith(value_ref, name_operand_index, name) };
}

#[derive(Clone)]
pub(crate) enum Value<'ctx> {
    MDNode(MDNode<'ctx>),
    Function(Function<'ctx>),
    Other(NonNull<LLVMValue>),
}

impl std::fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value_to_string = |value: NonNull<LLVMValue>| {
            Message {
                ptr: unsafe { LLVMPrintValueToString(value.as_ptr()) },
            }
            .as_string_lossy()
            .to_string()
        };
        match self {
            Self::MDNode(node) => f
                .debug_struct("MDNode")
                .field("value", &value_to_string(node.value))
                .finish(),
            Self::Function(fun) => f
                .debug_struct("Function")
                .field("value", &value_to_string(fun.value))
                .finish(),
            Self::Other(value) => f
                .debug_struct("Other")
                .field("value", &value_to_string(*value))
                .finish(),
        }
    }
}

impl Value<'_> {
    pub(crate) fn from_raw(value: LLVMValueRef) -> Result<Self, LLVMTypeError> {
        let value = NonNull::new(value).ok_or_else(|| LLVMTypeError::NullPtr("Value"))?;
        Ok(if unsafe { !LLVMIsAMDNode(value.as_ptr()).is_null() } {
            let mdnode = unsafe { MDNode::from_non_null(value) };
            Value::MDNode(mdnode)
        } else if unsafe { !LLVMIsAFunction(value.as_ptr()).is_null() } {
            let function = unsafe { Function::from_non_null(value) };
            Value::Function(function)
        } else {
            Value::Other(value)
        })
    }

    pub(crate) fn metadata_entries(&self) -> Option<MetadataEntries> {
        let value = match self {
            Value::MDNode(node) => node.value,
            Value::Function(f) => f.value,
            Value::Other(value) => *value,
        };
        MetadataEntries::new(value)
    }

    pub(crate) fn operands(&self) -> Option<impl Iterator<Item = LLVMValueRef>> {
        let value = match self {
            Value::MDNode(node) => Some(node.value.as_ptr()),
            Value::Function(f) => Some(f.value.as_ptr()),
            Value::Other(value) if unsafe { !LLVMIsAUser(value.as_ptr()).is_null() } => {
                Some(value.as_ptr())
            }
            _ => None,
        };

        value.map(|value| unsafe {
            (0..LLVMGetNumOperands(value)).map(move |i| LLVMGetOperand(value, i.cast_unsigned()))
        })
    }
}

pub(crate) enum Metadata<'ctx> {
    DICompositeType(DICompositeType<'ctx>),
    DIDerivedType(DIDerivedType<'ctx>),
    DISubprogram(DISubprogram<'ctx>),
    Other(#[expect(dead_code)] NonNull<LLVMValue>),
}

impl Metadata<'_> {
    /// Constructs a new [`Metadata`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `Metadata`](https://llvm.org/doxygen/classllvm_1_1Metadata.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    pub(crate) unsafe fn from_raw(value: LLVMValueRef) -> Result<Self, LLVMTypeError> {
        let value = NonNull::new(value).ok_or_else(|| LLVMTypeError::NullPtr("Value"))?;
        Ok(unsafe { Self::from_non_null(value) })
    }

    /// Constructs a new [`Metadata`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `Metadata`](https://llvm.org/doxygen/classllvm_1_1Metadata.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    unsafe fn from_non_null(value: NonNull<LLVMValue>) -> Self {
        unsafe {
            let metadata = LLVMValueAsMetadata(value.as_ptr());

            match LLVMGetMetadataKind(metadata) {
                LLVMMetadataKind::LLVMDICompositeTypeMetadataKind => {
                    let di_composite_type = DICompositeType::from_non_null_unchecked(value);
                    Metadata::DICompositeType(di_composite_type)
                }
                LLVMMetadataKind::LLVMDIDerivedTypeMetadataKind => {
                    let di_derived_type = DIDerivedType::from_non_null_unchecked(value);
                    Metadata::DIDerivedType(di_derived_type)
                }
                LLVMMetadataKind::LLVMDISubprogramMetadataKind => {
                    let di_subprogram = DISubprogram::from_non_null_unchecked(value);
                    Metadata::DISubprogram(di_subprogram)
                }
                LLVMMetadataKind::LLVMDIGlobalVariableMetadataKind
                | LLVMMetadataKind::LLVMDICommonBlockMetadataKind
                | LLVMMetadataKind::LLVMMDStringMetadataKind
                | LLVMMetadataKind::LLVMConstantAsMetadataMetadataKind
                | LLVMMetadataKind::LLVMLocalAsMetadataMetadataKind
                | LLVMMetadataKind::LLVMDistinctMDOperandPlaceholderMetadataKind
                | LLVMMetadataKind::LLVMMDTupleMetadataKind
                | LLVMMetadataKind::LLVMDILocationMetadataKind
                | LLVMMetadataKind::LLVMDIExpressionMetadataKind
                | LLVMMetadataKind::LLVMDIGlobalVariableExpressionMetadataKind
                | LLVMMetadataKind::LLVMGenericDINodeMetadataKind
                | LLVMMetadataKind::LLVMDISubrangeMetadataKind
                | LLVMMetadataKind::LLVMDIEnumeratorMetadataKind
                | LLVMMetadataKind::LLVMDIBasicTypeMetadataKind
                | LLVMMetadataKind::LLVMDISubroutineTypeMetadataKind
                | LLVMMetadataKind::LLVMDIFileMetadataKind
                | LLVMMetadataKind::LLVMDICompileUnitMetadataKind
                | LLVMMetadataKind::LLVMDILexicalBlockMetadataKind
                | LLVMMetadataKind::LLVMDILexicalBlockFileMetadataKind
                | LLVMMetadataKind::LLVMDINamespaceMetadataKind
                | LLVMMetadataKind::LLVMDIModuleMetadataKind
                | LLVMMetadataKind::LLVMDITemplateTypeParameterMetadataKind
                | LLVMMetadataKind::LLVMDITemplateValueParameterMetadataKind
                | LLVMMetadataKind::LLVMDILocalVariableMetadataKind
                | LLVMMetadataKind::LLVMDILabelMetadataKind
                | LLVMMetadataKind::LLVMDIObjCPropertyMetadataKind
                | LLVMMetadataKind::LLVMDIImportedEntityMetadataKind
                | LLVMMetadataKind::LLVMDIMacroMetadataKind
                | LLVMMetadataKind::LLVMDIMacroFileMetadataKind
                | LLVMMetadataKind::LLVMDIStringTypeMetadataKind
                | LLVMMetadataKind::LLVMDIGenericSubrangeMetadataKind
                | LLVMMetadataKind::LLVMDIArgListMetadataKind
                | LLVMMetadataKind::LLVMDIAssignIDMetadataKind => Metadata::Other(value),
                #[cfg(not(feature = "llvm-20"))]
                LLVMMetadataKind::LLVMDISubrangeTypeMetadataKind
                | LLVMMetadataKind::LLVMDIFixedPointTypeMetadataKind => Metadata::Other(value),
            }
        }
    }
}

impl<'ctx> From<MDNode<'ctx>> for Metadata<'ctx> {
    fn from(md_node: MDNode<'_>) -> Self {
        // SAFETY: `MDNode` is a subclass of `Metadata`.
        unsafe { Self::from_non_null(md_node.value) }
    }
}

/// Represents a metadata node.
#[derive(Clone)]
pub(crate) struct MDNode<'ctx> {
    pub(super) value: NonNull<LLVMValue>,
    _marker: PhantomData<&'ctx ()>,
}

impl MDNode<'_> {
    /// Constructs a new [`MDNode`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `MDNode`](https://llvm.org/doxygen/classllvm_1_1MDNode.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    pub(crate) unsafe fn from_raw(value_ref: LLVMValueRef) -> Result<Self, LLVMTypeError> {
        let value_ref = NonNull::new(value_ref).ok_or_else(|| LLVMTypeError::NullPtr("MDNode"))?;
        Ok(Self {
            value: value_ref,
            _marker: PhantomData,
        })
    }

    unsafe fn from_non_null(value: NonNull<LLVMValue>) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    /// Constructs an empty metadata node.
    pub(crate) fn empty(context: &LLVMContext) -> Result<Self, LLVMTypeError> {
        let metadata =
            unsafe { LLVMMDNodeInContext2(context.as_mut_ptr(), core::ptr::null_mut(), 0) };
        let value = unsafe { LLVMMetadataAsValue(context.as_mut_ptr(), metadata) };
        unsafe { Self::from_raw(value) }
    }

    /// Constructs a new metadata node from an array of [`DIType`] elements.
    ///
    /// This function is used to create composite metadata structures, such as
    /// arrays or tuples of different types or values, which can then be used
    /// to represent complex data structures within the metadata system.
    pub(crate) fn with_elements(
        context: &LLVMContext,
        elements: &[DIType<'_>],
    ) -> Result<Self, LLVMTypeError> {
        let metadata = unsafe {
            let mut elements: Vec<LLVMMetadataRef> = elements
                .iter()
                .map(|di_type| LLVMValueAsMetadata(di_type.value.as_ptr()))
                .collect();
            LLVMMDNodeInContext2(
                context.as_mut_ptr(),
                elements.as_mut_slice().as_mut_ptr(),
                elements.len(),
            )
        };
        let value = unsafe { LLVMMetadataAsValue(context.as_mut_ptr(), metadata) };
        unsafe { Self::from_raw(value) }
    }
}

pub(crate) struct MetadataEntries {
    entries: NonNull<LLVMValueMetadataEntry>,
    count: u32,
}

impl MetadataEntries {
    pub(crate) fn new(v: NonNull<LLVMValue>) -> Option<Self> {
        if unsafe {
            LLVMIsAGlobalObject(v.as_ptr()).is_null() && LLVMIsAInstruction(v.as_ptr()).is_null()
        } {
            return None;
        }

        let mut count = 0;
        let entries = unsafe { LLVMGlobalCopyAllMetadata(v.as_ptr(), &mut count) };
        NonNull::new(entries).map(|entries| Self {
            entries,
            count: count.try_into().unwrap(),
        })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (LLVMMetadataRef, u32)> + '_ {
        let Self { entries, count } = self;
        (0..*count).map(|index| unsafe {
            (
                LLVMValueMetadataEntriesGetMetadata(entries.as_ptr(), index),
                LLVMValueMetadataEntriesGetKind(entries.as_ptr(), index),
            )
        })
    }
}

impl Drop for MetadataEntries {
    fn drop(&mut self) {
        unsafe {
            LLVMDisposeValueMetadataEntries(self.entries.as_ptr());
        }
    }
}

pub(crate) struct BasicBlockIter<'ctx> {
    current: Option<LLVMBasicBlockRef>,
    _lifetime: PhantomData<&'ctx ()>,
}

impl Iterator for BasicBlockIter<'_> {
    type Item = LLVMBasicBlockRef;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.inspect(|&basic_block| {
            // SAFETY: `LLVMBasicBlockRef` maps to exactly one LLVM C++ type
            // (`BasicBlock`), so there is no possibility of a mismatch.
            let current = unsafe { LLVMGetNextBasicBlock(basic_block) };
            self.current = NonNull::new(current).map(|basic_block| basic_block.as_ptr());
        })
    }
}

/// Represents a function.
#[derive(Clone)]
pub(crate) struct Function<'ctx> {
    pub value: NonNull<LLVMValue>,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> Function<'ctx> {
    /// Constructs a new [`Function`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `Function`](https://llvm.org/doxygen/classllvm_1_1Function.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    pub(crate) unsafe fn from_raw(value_ref: LLVMValueRef) -> Result<Self, LLVMTypeError> {
        let value = NonNull::new(value_ref).ok_or_else(|| LLVMTypeError::NullPtr("Function"))?;
        Ok(Self {
            value,
            _marker: PhantomData,
        })
    }

    unsafe fn from_non_null(value: NonNull<LLVMValue>) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    pub(crate) fn name(&self) -> &[u8] {
        symbol_name(self.value.as_ptr())
    }

    pub(crate) fn params(&self) -> impl Iterator<Item = LLVMValueRef> {
        let value = self.value.as_ptr();
        let params_count = unsafe { LLVMCountParams(value) };
        (0..params_count).map(move |i| unsafe { LLVMGetParam(value, i) })
    }

    pub(crate) fn basic_blocks(&self) -> impl Iterator<Item = LLVMBasicBlockRef> + '_ {
        // SAFETY: We are sure that the provided `LLVMValueRef` is a
        // `Function`.
        let current = unsafe { LLVMGetFirstBasicBlock(self.value.as_ptr()) };
        let current = NonNull::new(current).map(|function| function.as_ptr());
        BasicBlockIter {
            current,
            _lifetime: PhantomData,
        }
    }

    pub(crate) fn subprogram(&self, context: LLVMContextRef) -> Option<DISubprogram<'ctx>> {
        let subprogram = unsafe { LLVMGetSubprogram(self.value.as_ptr()) };
        (!subprogram.is_null()).then(|| unsafe {
            DISubprogram::from_raw(LLVMMetadataAsValue(context, subprogram)).expect(
                "subprogram belonging to a non-null function should be a correct non-null pointer",
            )
        })
    }

    pub(crate) fn set_subprogram(&mut self, subprogram: &DISubprogram<'_>) {
        unsafe {
            LLVMSetSubprogram(
                self.value.as_ptr(),
                LLVMValueAsMetadata(subprogram.value.as_ptr()),
            )
        };
    }
}
