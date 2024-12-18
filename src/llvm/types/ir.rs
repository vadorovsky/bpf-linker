use std::{
    ffi::{CString, NulError},
    marker::PhantomData,
    ptr::NonNull,
};

use llvm_sys::{
    core::{
        LLVMCountParams, LLVMDisposeValueMetadataEntries, LLVMGetNumOperands, LLVMGetOperand,
        LLVMGetParam, LLVMGlobalCopyAllMetadata, LLVMIsAFunction, LLVMIsAGlobalObject,
        LLVMIsAInstruction, LLVMIsAMDNode, LLVMIsAUser, LLVMMDNodeInContext2,
        LLVMMDStringInContext2, LLVMMetadataAsValue, LLVMPrintValueToString,
        LLVMReplaceMDNodeOperandWith, LLVMValueAsMetadata, LLVMValueMetadataEntriesGetKind,
        LLVMValueMetadataEntriesGetMetadata,
    },
    debuginfo::{LLVMGetMetadataKind, LLVMGetSubprogram, LLVMMetadataKind, LLVMSetSubprogram},
    prelude::{
        LLVMBasicBlockRef, LLVMContextRef, LLVMMetadataRef, LLVMValueMetadataEntry, LLVMValueRef,
    },
};

use crate::llvm::{
    iter::IterBasicBlocks as _,
    symbol_name,
    types::di::{DICompositeType, DIDerivedType, DISubprogram, DIType},
    Message,
};

pub(crate) fn replace_name(
    value_ref: LLVMValueRef,
    context: LLVMContextRef,
    name_operand_index: u32,
    name: &str,
) -> Result<(), NulError> {
    let cstr = CString::new(name)?;
    let name = unsafe { LLVMMDStringInContext2(context, cstr.as_ptr(), name.len()) };
    unsafe { LLVMReplaceMDNodeOperandWith(value_ref, name_operand_index, name) };
    Ok(())
}

#[derive(Clone)]
pub enum Value<'ctx> {
    MDNode(MDNode<'ctx>),
    Function(Function<'ctx>),
    Other(LLVMValueRef),
}

impl std::fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value_to_string = |value| {
            Message {
                ptr: unsafe { LLVMPrintValueToString(value) },
            }
            .as_c_str()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
        };
        match self {
            Self::MDNode(node) => f
                .debug_struct("MDNode")
                .field("value", &value_to_string(node.value_ref))
                .finish(),
            Self::Function(fun) => f
                .debug_struct("Function")
                .field("value", &value_to_string(fun.value_ref))
                .finish(),
            Self::Other(value) => f
                .debug_struct("Other")
                .field("value", &value_to_string(*value))
                .finish(),
        }
    }
}

impl Value<'_> {
    pub fn new(value: LLVMValueRef) -> Self {
        if unsafe { !LLVMIsAMDNode(value).is_null() } {
            let mdnode = unsafe { MDNode::from_value_ref(value) };
            return Value::MDNode(mdnode);
        } else if unsafe { !LLVMIsAFunction(value).is_null() } {
            return Value::Function(unsafe { Function::from_value_ref(value) });
        }
        Value::Other(value)
    }

    pub fn metadata_entries(&self) -> Option<MetadataEntries> {
        let value = match self {
            Value::MDNode(node) => node.value_ref,
            Value::Function(f) => f.value_ref,
            Value::Other(value) => *value,
        };
        MetadataEntries::new(value)
    }

    pub fn operands(&self) -> Option<impl Iterator<Item = LLVMValueRef>> {
        let value = match self {
            Value::MDNode(node) => Some(node.value_ref),
            Value::Function(f) => Some(f.value_ref),
            Value::Other(value) if unsafe { !LLVMIsAUser(*value).is_null() } => Some(*value),
            _ => None,
        };

        value.map(|value| unsafe {
            (0..LLVMGetNumOperands(value)).map(move |i| LLVMGetOperand(value, i as u32))
        })
    }
}

pub enum Metadata<'ctx> {
    DICompositeType(DICompositeType<'ctx>),
    DIDerivedType(DIDerivedType<'ctx>),
    DISubprogram(DISubprogram<'ctx>),
    Other(#[allow(dead_code)] LLVMValueRef),
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
    pub(crate) unsafe fn from_value_ref(value: LLVMValueRef) -> Self {
        let metadata = LLVMValueAsMetadata(value);

        match unsafe { LLVMGetMetadataKind(metadata) } {
            LLVMMetadataKind::LLVMDICompositeTypeMetadataKind => {
                let di_composite_type = unsafe { DICompositeType::from_value_ref(value) };
                Metadata::DICompositeType(di_composite_type)
            }
            LLVMMetadataKind::LLVMDIDerivedTypeMetadataKind => {
                let di_derived_type = unsafe { DIDerivedType::from_value_ref(value) };
                Metadata::DIDerivedType(di_derived_type)
            }
            LLVMMetadataKind::LLVMDISubprogramMetadataKind => {
                let di_subprogram = unsafe { DISubprogram::from_value_ref(value) };
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
        }
    }
}

impl<'ctx> TryFrom<MDNode<'ctx>> for Metadata<'ctx> {
    type Error = ();

    fn try_from(md_node: MDNode) -> Result<Self, Self::Error> {
        // FIXME: fail if md_node isn't a Metadata node
        Ok(unsafe { Self::from_value_ref(md_node.value_ref) })
    }
}

/// Represents a metadata node.
#[derive(Clone)]
pub struct MDNode<'ctx> {
    pub(super) value_ref: LLVMValueRef,
    _marker: PhantomData<&'ctx ()>,
}

impl MDNode<'_> {
    /// Constructs a new [`MDNode`] from the given `metadata`.
    ///
    /// # Safety
    ///
    /// This method assumes that the given `metadata` corresponds to a valid
    /// instance of [LLVM `MDNode`](https://llvm.org/doxygen/classllvm_1_1MDNode.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any validation checks.
    pub(crate) unsafe fn from_metadata_ref(
        context: LLVMContextRef,
        metadata: LLVMMetadataRef,
    ) -> Self {
        MDNode::from_value_ref(LLVMMetadataAsValue(context, metadata))
    }

    /// Constructs a new [`MDNode`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `MDNode`](https://llvm.org/doxygen/classllvm_1_1MDNode.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    pub(crate) unsafe fn from_value_ref(value_ref: LLVMValueRef) -> Self {
        Self {
            value_ref,
            _marker: PhantomData,
        }
    }

    /// Constructs an empty metadata node.
    pub fn empty(context: LLVMContextRef) -> Self {
        let metadata = unsafe { LLVMMDNodeInContext2(context, core::ptr::null_mut(), 0) };
        unsafe { Self::from_metadata_ref(context, metadata) }
    }

    /// Constructs a new metadata node from an array of [`DIType`] elements.
    ///
    /// This function is used to create composite metadata structures, such as
    /// arrays or tuples of different types or values, which can then be used
    /// to represent complex data structures within the metadata system.
    pub fn with_elements(context: LLVMContextRef, elements: &[DIType]) -> Self {
        let metadata = unsafe {
            let mut elements: Vec<LLVMMetadataRef> = elements
                .iter()
                .map(|di_type| LLVMValueAsMetadata(di_type.value_ref))
                .collect();
            LLVMMDNodeInContext2(
                context,
                elements.as_mut_slice().as_mut_ptr(),
                elements.len(),
            )
        };
        unsafe { Self::from_metadata_ref(context, metadata) }
    }
}

pub struct MetadataEntries {
    entries: *mut LLVMValueMetadataEntry,
    count: usize,
}

impl MetadataEntries {
    pub fn new(v: LLVMValueRef) -> Option<Self> {
        if unsafe { LLVMIsAGlobalObject(v).is_null() && LLVMIsAInstruction(v).is_null() } {
            return None;
        }

        let mut count = 0;
        let entries = unsafe { LLVMGlobalCopyAllMetadata(v, &mut count) };
        if entries.is_null() {
            return None;
        }

        Some(MetadataEntries { entries, count })
    }

    pub fn iter(&self) -> impl Iterator<Item = (LLVMMetadataRef, u32)> + '_ {
        (0..self.count).map(move |index| unsafe {
            (
                LLVMValueMetadataEntriesGetMetadata(self.entries, index as u32),
                LLVMValueMetadataEntriesGetKind(self.entries, index as u32),
            )
        })
    }
}

impl Drop for MetadataEntries {
    fn drop(&mut self) {
        unsafe {
            LLVMDisposeValueMetadataEntries(self.entries);
        }
    }
}

/// Represents a metadata node.
#[derive(Clone)]
pub struct Function<'ctx> {
    pub value_ref: LLVMValueRef,
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
    pub(crate) unsafe fn from_value_ref(value_ref: LLVMValueRef) -> Self {
        Self {
            value_ref,
            _marker: PhantomData,
        }
    }

    pub(crate) fn name(&self) -> &str {
        symbol_name(self.value_ref)
    }

    pub(crate) fn params(&self) -> impl Iterator<Item = LLVMValueRef> {
        let params_count = unsafe { LLVMCountParams(self.value_ref) };
        let value = self.value_ref;
        (0..params_count).map(move |i| unsafe { LLVMGetParam(value, i) })
    }

    pub(crate) fn basic_blocks(&self) -> impl Iterator<Item = LLVMBasicBlockRef> + '_ {
        self.value_ref.basic_blocks_iter()
    }

    pub(crate) fn subprogram(&self, context: LLVMContextRef) -> Option<DISubprogram<'ctx>> {
        let subprogram = unsafe { LLVMGetSubprogram(self.value_ref) };
        NonNull::new(subprogram).map(|_| unsafe {
            DISubprogram::from_value_ref(LLVMMetadataAsValue(context, subprogram))
        })
    }

    pub(crate) fn set_subprogram(&mut self, subprogram: &DISubprogram) {
        unsafe { LLVMSetSubprogram(self.value_ref, LLVMValueAsMetadata(subprogram.value_ref)) };
    }
}
