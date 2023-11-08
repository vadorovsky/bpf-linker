use std::marker::PhantomData;

use llvm_sys::{
    core::{
        LLVMGetMDNodeNumOperands, LLVMGetNumOperands, LLVMGetOperand, LLVMGlobalCopyAllMetadata,
        LLVMGlobalSetMetadata, LLVMIsAGlobalObject, LLVMIsAInstruction, LLVMIsAMDNode, LLVMIsAUser,
        LLVMMDNodeInContext2, LLVMMetadataAsValue, LLVMPrintValueToString, LLVMSetMetadata,
        LLVMValueAsMetadata, LLVMValueMetadataEntriesGetKind, LLVMValueMetadataEntriesGetMetadata,
    },
    debuginfo::{LLVMGetMetadataKind, LLVMMetadataKind},
    prelude::*,
};

use super::{
    di::{DICommonBlock, DICompositeType, DIDerivedType, DIGlobalVariable, DISubprogram, DIType},
    symbol_name, Message,
};

pub enum ValueType<'a> {
    User(User<'a>),
    GlobalObject(GlobalObject<'a>),
    Instruction(Instruction<'a>),
    MDNode(MDNode<'a>),
    Unknown(Value<'a>),
}

pub struct Value<'a> {
    pub(crate) value: LLVMValueRef,
    _marker: PhantomData<&'a ()>,
}

impl<'a> Value<'a> {
    pub fn new(value: LLVMValueRef) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    pub fn as_message(&self) -> Message {
        Message {
            ptr: unsafe { LLVMPrintValueToString(self.value) },
        }
    }

    pub fn into_value_type(self) -> ValueType<'a> {
        if unsafe { !LLVMIsAUser(self.value).is_null() } {
            let user = unsafe { User::from_value_ref(self.value) };
            return ValueType::User(user);
        }
        if unsafe { !LLVMIsAGlobalObject(self.value).is_null() } {
            let global_object = unsafe { GlobalObject::from_value_ref(self.value) };
            return ValueType::GlobalObject(global_object);
        }
        if unsafe { !LLVMIsAInstruction(self.value).is_null() } {
            let instruction = unsafe { Instruction::from_value_ref(self.value) };
            return ValueType::Instruction(instruction);
        }
        if unsafe { !LLVMIsAMDNode(self.value).is_null() } {
            let mdnode = unsafe { MDNode::from_value_ref(self.value) };
            return ValueType::MDNode(mdnode);
        }
        ValueType::Unknown(self)
    }

    /// # Safety
    fn iter_metadata_copy(&self, ctx: LLVMContextRef) -> impl Iterator<Item = (u32, Metadata)> {
        let mut count = 0;
        let entries = unsafe { LLVMGlobalCopyAllMetadata(self.value, &mut count) };
        (0..count).map(move |index| {
            let kind = unsafe { LLVMValueMetadataEntriesGetKind(entries, index as u32) };
            let metadata = unsafe {
                let metadata_ref = LLVMValueMetadataEntriesGetMetadata(entries, index as u32);
                let value_ref = LLVMMetadataAsValue(ctx, metadata_ref);
                // SAFETY: `Metadata` contains a `Value` as the only field. `Value`
                // contains a reference to `LLVMValue` as the only field and the
                // following cast is the only way to let Rust know that we are
                // yielding a reference to an existing value instead of creating a
                // new one. There is no other way to return `&Metadata` here.
                Metadata::from_value_ref(value_ref)
                // &*(value_ref as *const Metadata<'a>)
            };
            (kind, metadata)
        })
    }

    fn iter_mut_metadata_copy(
        &mut self,
        ctx: LLVMContextRef,
    ) -> impl Iterator<Item = (u32, Metadata)> {
        let mut count = 0;
        let entries = unsafe { LLVMGlobalCopyAllMetadata(self.value, &mut count) };
        (0..count).map(move |index| {
            let kind = unsafe { LLVMValueMetadataEntriesGetKind(entries, index as u32) };
            let metadata = unsafe {
                let metadata_ref = LLVMValueMetadataEntriesGetMetadata(entries, index as u32);
                let value_ref = LLVMMetadataAsValue(ctx, metadata_ref);
                // SAFETY: `Metadata` contains a `Value` as the only field. `Value`
                // contains a reference to `LLVMValue` as the only field and the
                // following cast is the only way to let Rust know that we are
                // yielding a reference to an existing value instead of creating a
                // new one. There is no other way to return `&Metadata` here.
                Metadata::from_value_ref(value_ref)
                // &mut *(value_ref as *mut Metadata<'a>)
            };
            (kind, metadata)
        })
    }

    pub fn num_operands(&self) -> i32 {
        unsafe { LLVMGetNumOperands(self.value) }
    }

    pub fn operands(&'a self) -> impl Iterator<Item = &'a Value> + 'a {
        // SAFETY: Calling `LLVMGetOperand` on `Value` and all its child
        // classes is valid.
        // Calling `LLVMGetOperand` doesn't mutate the underlying value unless
        // the operand is further modified, which would require returning a
        // mutable reference.
        // `Value` contains a reference to `LLVMValue` as the only field and
        // the following cast is the only way to let Rust know that we are
        // yielding a reference to an existing value instead of creating a new
        // one. There is no other way to return `&Value` here.
        (0..self.num_operands()).map(move |i| unsafe {
            let operand_ref = LLVMGetOperand(self.value as *const _ as *mut _, i as u32);
            // let value = Value::new(operand_ref);
            // &value
            &*(operand_ref as *const Value<'a>)
        })
    }

    pub fn symbol_name<'b>(&self) -> &'b str {
        symbol_name(self.value)
    }
}

pub struct User<'a> {
    pub value: Value<'a>,
}

impl<'a> User<'a> {
    /// Constructs a new [`User`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `User`](https://llvm.org/doxygen/classllvm_1_1User.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    pub(crate) unsafe fn from_value_ref(value: LLVMValueRef) -> Self {
        let value = Value::new(value);
        Self { value }
    }

    pub fn as_message(&self) -> Message {
        self.value.as_message()
    }

    pub fn operands(&'a self) -> impl Iterator<Item = &'a Value> + '_ {
        self.value.operands()
    }

    pub fn symbol_name<'b>(&self) -> &'b str {
        self.value.symbol_name()
    }
}

/// Represents LLVM IR global object.
pub struct GlobalObject<'a> {
    pub value: Value<'a>,
}

impl<'a> GlobalObject<'a> {
    /// Constructs a new [`GlobalObject`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `GlobalObject`](https://llvm.org/doxygen/classllvm_1_1GlobalObject.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    pub(crate) unsafe fn from_value_ref(value: LLVMValueRef) -> Self {
        let value = Value::new(value);
        Self { value }
    }

    pub fn iter_metadata_copy(&self, ctx: LLVMContextRef) -> impl Iterator<Item = (u32, Metadata)> {
        self.value.iter_metadata_copy(ctx)
    }

    pub fn iter_mut_metadata_copy(
        &'a mut self,
        ctx: LLVMContextRef,
    ) -> impl Iterator<Item = (u32, Metadata)> {
        self.value.iter_mut_metadata_copy(ctx)
    }

    pub fn set_metadata(&mut self, kind: u32, metadata: &Metadata) {
        unsafe {
            let metadata_ref = LLVMValueAsMetadata(metadata.value.value);
            LLVMGlobalSetMetadata(self.value.value, kind, metadata_ref)
        }
    }
}

/// Represents LLVM IR instruction.
pub struct Instruction<'a> {
    pub value: Value<'a>,
}

impl<'a> Instruction<'a> {
    /// Constructs a new [`Metadata`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `Instruction`](https://llvm.org/doxygen/classllvm_1_1Instruction.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    pub(crate) unsafe fn from_value_ref(value: LLVMValueRef) -> Self {
        let value = Value::new(value);
        Self { value }
    }

    pub fn iter_metadata_copy(&self, ctx: LLVMContextRef) -> impl Iterator<Item = (u32, Metadata)> {
        self.value.iter_metadata_copy(ctx)
    }

    pub fn set_metadata(&mut self, kind: u32, metadata: &Metadata) {
        unsafe { LLVMSetMetadata(self.value.value, kind, metadata.value.value) };
    }
}

pub enum MetadataKind<'a> {
    DICompositeType(DICompositeType<'a>),
    DIGlobalVariable(DIGlobalVariable<'a>),
    DICommonBlock(DICommonBlock<'a>),
    DIDerivedType(DIDerivedType<'a>),
    DISubprogram(DISubprogram<'a>),
    Unknown(Metadata<'a>),
}

/// Represents LLVM IR metadata.
pub struct Metadata<'a> {
    // pub(crate) metadata: &'a mut LLVMOpaqueMetadata,
    pub value: Value<'a>,
}

impl<'a> Metadata<'a> {
    // /// Constructs a new [`Metadata`] from the given raw pointer.
    // pub(crate) fn from_metadata_ref(context: LLVMContextRef, metadata: LLVMMetadataRef) -> Self {
    //     let value = unsafe { LLVMMetadataAsValue(context, metadata) };
    //     let value = Value::new(value);
    //     // Self { metadata, value }
    //     Self { value }
    // }

    /// Constructs a new [`Metadata`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `Metadata`](https://llvm.org/doxygen/classllvm_1_1Metadata.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    pub(crate) unsafe fn from_value_ref(value: LLVMValueRef) -> Self {
        // let metadata = LLVMValueAsMetadata(value);
        let value = Value::new(value);
        // Self { metadata, value }
        Self { value }
    }

    pub fn as_message(&self) -> Message {
        self.value.as_message()
    }

    pub fn metadata_kind(&self) -> LLVMMetadataKind {
        unsafe {
            let metadata_ref = LLVMValueAsMetadata(self.value.value);
            LLVMGetMetadataKind(metadata_ref)
        }
    }

    pub fn into_metadata_kind(&self) -> MetadataKind {
        let metadata_ref = unsafe { LLVMValueAsMetadata(self.value.value) };
        match unsafe { LLVMGetMetadataKind(metadata_ref) } {
            LLVMMetadataKind::LLVMDICompositeTypeMetadataKind => {
                let di_composite_type =
                    unsafe { DICompositeType::from_value_ref(self.value.value) };
                MetadataKind::DICompositeType(di_composite_type)
            }
            LLVMMetadataKind::LLVMDIGlobalVariableMetadataKind => {
                let di_global_variale =
                    unsafe { DIGlobalVariable::from_value_ref(self.value.value) };
                MetadataKind::DIGlobalVariable(di_global_variale)
            }
            LLVMMetadataKind::LLVMDICommonBlockMetadataKind => {
                let di_common_block = unsafe { DICommonBlock::from_value_ref(self.value.value) };
                MetadataKind::DICommonBlock(di_common_block)
            }
            LLVMMetadataKind::LLVMDIDerivedTypeMetadataKind => {
                let di_derived_type = unsafe { DIDerivedType::from_value_ref(self.value.value) };
                MetadataKind::DIDerivedType(di_derived_type)
            }
            LLVMMetadataKind::LLVMDISubprogramMetadataKind => {
                let di_subprogram = unsafe { DISubprogram::from_value_ref(self.value.value) };
                MetadataKind::DISubprogram(di_subprogram)
            }
            LLVMMetadataKind::LLVMMDStringMetadataKind
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
            | LLVMMetadataKind::LLVMDIAssignIDMetadataKind => unimplemented!(),
        }
    }
}

/// Represents a metadata node.
pub struct MDNode<'a> {
    pub metadata: Metadata<'a>,
}

impl<'a> MDNode<'a> {
    // /// Constructs a new [`MDNode`] from the given `metadata`.
    // ///
    // /// # Safety
    // ///
    // /// This method assumes that the given `metadata` corresponds to a valid
    // /// instance of [LLVM `MDNode`](https://llvm.org/doxygen/classllvm_1_1MDNode.html).
    // /// It's the caller's responsibility to ensure this invariant, as this
    // /// method doesn't perform any validation checks.
    // pub(crate) unsafe fn from_metadata_ref(
    //     context: LLVMContextRef,
    //     metadata: LLVMMetadataRef,
    // ) -> Self {
    //     let metadata = Metadata::from_metadata_ref(context, metadata);
    //     Self { metadata }
    // }

    /// Constructs a new [`Metadata`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `MDNode`](https://llvm.org/doxygen/classllvm_1_1MDNode.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    pub(crate) unsafe fn from_value_ref(value: LLVMValueRef) -> Self {
        let metadata = Metadata::from_value_ref(value);
        Self { metadata }
    }

    pub fn as_message(&self) -> Message {
        self.metadata.as_message()
    }

    /// Constructs an empty metadata node.
    pub fn empty(ctx: LLVMContextRef) -> Self {
        let metadata = unsafe {
            let metadata_ref = LLVMMDNodeInContext2(ctx, core::ptr::null_mut(), 0);
            let value_ref = LLVMMetadataAsValue(ctx, metadata_ref);
            Metadata::from_value_ref(value_ref)
        };
        Self { metadata }
    }

    pub fn metadata_kind(&self) -> LLVMMetadataKind {
        self.metadata.metadata_kind()
    }

    /// Returns the number of operands in the given [`MDNode`].
    pub fn num_operands(&self) -> u32 {
        unsafe { LLVMGetMDNodeNumOperands(self.metadata.value.value) }
    }

    pub fn operands(&'a self) -> impl Iterator<Item = &'a Value> + '_ {
        self.metadata.value.operands()
    }

    pub fn symbol_name<'b>(&self) -> &'b str {
        self.metadata.value.symbol_name()
    }

    /// Constructs a new metadata node from an array of [`DIType`] elements.
    ///
    /// This function is used to create composite metadata structures, such as
    /// arrays or tuples of different types or values, which can then be used
    /// to represent complex data structures within the metadata system.
    pub fn with_elements(ctx: LLVMContextRef, elements: &[DIType]) -> Self {
        let metadata = unsafe {
            let mut elements: Vec<LLVMMetadataRef> = elements
                .iter()
                .map(|di_type| {
                    LLVMValueAsMetadata(di_type.di_scope.di_node.md_node.metadata.value.value)
                })
                .collect();
            let metadata_ref =
                LLVMMDNodeInContext2(ctx, elements.as_mut_slice().as_mut_ptr(), elements.len());
            let value_ref = LLVMMetadataAsValue(ctx, metadata_ref);
            Metadata::from_value_ref(value_ref)
        };
        Self { metadata }
    }
}
