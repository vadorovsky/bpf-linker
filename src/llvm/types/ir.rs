use std::{
    ffi::{CString, NulError},
    fmt::Debug,
    iter::Skip,
    marker::PhantomData,
    ptr::NonNull,
};

use llvm_sys::{
    core::{
        LLVMConstInt, LLVMConstIntGetZExtValue, LLVMContextCreate, LLVMContextDispose,
        LLVMContextSetDiagnosticHandler, LLVMCountParams, LLVMCreateBuilderInContext,
        LLVMDisposeModule, LLVMDisposeValueMetadataEntries, LLVMGetGEPSourceElementType,
        LLVMGetIntrinsicDeclaration, LLVMGetNextInstruction, LLVMGetNumOperands, LLVMGetOperand,
        LLVMGetParam, LLVMGlobalCopyAllMetadata, LLVMGlobalGetValueType, LLVMHasMetadata,
        LLVMInt32Type, LLVMInt8TypeInContext, LLVMIsAConstantInt, LLVMIsADbgVariableIntrinsic,
        LLVMIsAFunction, LLVMIsAGetElementPtrInst, LLVMIsAGlobalObject, LLVMIsAInstruction,
        LLVMIsAIntToPtrInst, LLVMIsALoadInst, LLVMIsAMDNode, LLVMIsAUser, LLVMMDNodeInContext2,
        LLVMMDStringInContext2, LLVMMetadataAsValue, LLVMModuleCreateWithNameInContext,
        LLVMPrintDbgRecordToString, LLVMPrintTypeToString, LLVMPrintValueToString,
        LLVMReplaceMDNodeOperandWith, LLVMTypeOf, LLVMValueAsMetadata,
        LLVMValueMetadataEntriesGetKind, LLVMValueMetadataEntriesGetMetadata,
    },
    debuginfo::{
        LLVMCreateDIBuilder, LLVMGetMetadataKind, LLVMGetSubprogram, LLVMMetadataKind,
        LLVMSetSubprogram,
    },
    prelude::{
        LLVMBasicBlockRef, LLVMBuilderRef, LLVMContextRef, LLVMDIBuilderRef, LLVMDbgRecordRef,
        LLVMMetadataRef, LLVMModuleRef, LLVMTypeRef, LLVMValueMetadataEntry, LLVMValueRef,
    },
};

use crate::llvm::{
    self, symbol_name,
    types::di::{DICompositeType, DIDerivedType, DISubprogram, DIType},
    LLVMDiagnosticHandler, Message,
};

use super::di::DILocalVariable;

pub(crate) fn replace_name(
    value_ref: LLVMValueRef,
    context: &mut Context,
    name_operand_index: u32,
    name: &str,
) -> Result<(), NulError> {
    let cstr = CString::new(name)?;
    let name = unsafe { LLVMMDStringInContext2(context.context_ref(), cstr.as_ptr(), name.len()) };
    unsafe { LLVMReplaceMDNodeOperandWith(value_ref, name_operand_index, name) };
    Ok(())
}

#[derive(Clone)]
pub struct Context<'ctx> {
    context_ref: LLVMContextRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> Drop for Context<'ctx> {
    fn drop(&mut self) {
        tracing::debug!("dropping context");
        unsafe { LLVMContextDispose(self.context_ref) }
    }
}

impl<'ctx> Context<'ctx> {
    pub fn new() -> Self {
        let context_ref = unsafe { LLVMContextCreate() };
        Self {
            context_ref,
            _marker: PhantomData,
        }
    }

    pub fn context_ref(&self) -> LLVMContextRef {
        self.context_ref
    }

    pub fn create_builder(&mut self) -> LLVMBuilderRef {
        unsafe { LLVMCreateBuilderInContext(self.context_ref) }
    }

    pub fn create_module(&mut self, name: &str) -> Module<'ctx> {
        let c_name = CString::new(name).unwrap();
        let module_ref =
            unsafe { LLVMModuleCreateWithNameInContext(c_name.as_ptr(), self.context_ref) };

        Module {
            module_ref,
            _marker: PhantomData,
        }
    }

    pub fn set_diagnostic_handler<T: LLVMDiagnosticHandler>(&mut self, diagnostic_handler: &mut T) {
        unsafe {
            LLVMContextSetDiagnosticHandler(
                self.context_ref,
                Some(llvm::diagnostic_handler::<T>),
                diagnostic_handler as *mut _ as _,
            )
        }
    }
}

#[derive(Clone)]
pub struct Module<'ctx> {
    module_ref: LLVMModuleRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> Drop for Module<'ctx> {
    fn drop(&mut self) {
        tracing::debug!("dropping module");
        unsafe { LLVMDisposeModule(self.module_ref) }
    }
}

impl<'ctx> Module<'ctx> {
    pub fn create_di_builder(&mut self) -> LLVMDIBuilderRef {
        unsafe { LLVMCreateDIBuilder(self.module_ref) }
    }

    pub fn module_ref(&self) -> LLVMModuleRef {
        self.module_ref
    }
}

pub trait ValueRef {
    fn value_ref(&self) -> LLVMValueRef;
}

pub struct OperandsIterator<'ctx> {
    curr: u32,
    limit: u32,
    value_ref: LLVMValueRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> Iterator for OperandsIterator<'ctx> {
    type Item = Value<'ctx>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr < self.limit {
            let res = unsafe { Some(Value::new(LLVMGetOperand(self.value_ref, self.curr))) };
            self.curr += 1;
            res
        } else {
            None
        }
    }
}

impl<'ctx> OperandsIterator<'ctx> {
    pub(super) fn new(value_ref: LLVMValueRef) -> Self {
        let limit = unsafe { LLVMGetNumOperands(value_ref) as u32 };
        Self {
            curr: 0,
            limit,
            value_ref,
            _marker: PhantomData,
        }
    }

    pub(super) fn new_from(value_ref: LLVMValueRef, start: u32) -> Self {
        let limit = unsafe { LLVMGetNumOperands(value_ref) as u32 };
        Self {
            curr: start,
            limit,
            value_ref,
            _marker: PhantomData,
        }
    }
}

pub trait Operands: ValueRef {
    fn operands(&self) -> OperandsIterator {
        OperandsIterator::new(self.value_ref())
    }
}

#[derive(Clone)]
pub enum Value<'ctx> {
    Metadata(Metadata<'ctx>),
    User(User<'ctx>),
    Other(LLVMValueRef),
}

impl<'ctx> From<Instruction<'ctx>> for Value<'ctx> {
    fn from(value: Instruction<'ctx>) -> Self {
        Self::User(User::Instruction(value))
    }
}

impl<'ctx> From<GetElementPtrInst<'ctx>> for Value<'ctx> {
    fn from(value: GetElementPtrInst<'ctx>) -> Self {
        Self::User(User::Instruction(Instruction::GetElementPtrInst(value)))
    }
}

impl<'ctx> From<LoadInst<'ctx>> for Value<'ctx> {
    fn from(value: LoadInst<'ctx>) -> Self {
        Self::User(User::Instruction(Instruction::LoadInst(value)))
    }
}

impl<'ctx> std::fmt::Debug for Value<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self.value_ref();
        let value_string = Message {
            ptr: unsafe { LLVMPrintValueToString(value) },
        }
        .as_c_str()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        match self {
            Self::Metadata(metadata) => Metadata::fmt(metadata, f),
            Self::User(User::ConstantInt(_)) => f
                .debug_struct("ConstantInt")
                .field("value", &value_string)
                .finish(),
            Self::User(User::Function(_)) => f
                .debug_struct("Function")
                .field("value", &value_string)
                .finish(),
            Self::User(User::Instruction(_)) => f
                .debug_struct("Instruction")
                .field("value", &value_string)
                .finish(),
            Self::Other(_) => f
                .debug_struct("Other")
                .field("value", &value_string)
                .finish(),
        }
    }
}

impl<'ctx> ValueRef for Value<'ctx> {
    fn value_ref(&self) -> LLVMValueRef {
        match self {
            Self::Metadata(metadata) => metadata.value_ref(),
            Self::User(user) => user.value_ref(),
            Self::Other(value) => *value,
        }
    }
}

impl<'ctx> Value<'ctx> {
    pub fn new(value: LLVMValueRef) -> Self {
        if unsafe { !LLVMIsAMDNode(value).is_null() } {
            let mdnode = unsafe { MDNode::from_value_ref(value) };
            return Value::Metadata(Metadata::MDNode(mdnode));
        } else if unsafe { !LLVMIsAConstantInt(value).is_null() } {
            let constant_int = unsafe { ConstantInt::from_value_ref(value) };
            return Value::User(User::ConstantInt(constant_int));
        } else if unsafe { !LLVMIsAFunction(value).is_null() } {
            return Value::User(User::Function(unsafe { Function::from_value_ref(value) }));
        } else if unsafe { !LLVMIsAInstruction(value).is_null() } {
            let inst = Instruction::new(value);
            return Value::User(User::Instruction(inst));
        }
        Value::Other(value)
    }

    pub fn metadata_entries(&self) -> Option<MetadataEntries> {
        let value = self.value_ref();
        MetadataEntries::new(value)
    }

    pub fn operands(&self) -> Option<impl Iterator<Item = Value>> {
        match self {
            Value::Metadata(metadata) => Some(metadata.operands()),
            Value::User(user) => Some(user.operands()),
            _ => None,
        }
    }

    pub fn get_type(&self) -> Type<'_> {
        unsafe {
            let type_ref = LLVMTypeOf(self.value_ref());
            Type::from_type_ref(type_ref)
        }
    }
}

#[derive(Clone)]
pub struct BasicBlock<'ctx> {
    basic_block_ref: LLVMBasicBlockRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> BasicBlock<'ctx> {
    pub fn from_basic_block_ref(basic_block_ref: LLVMBasicBlockRef) -> Self {
        Self {
            basic_block_ref,
            _marker: PhantomData,
        }
    }

    pub fn basic_block_ref(&self) -> LLVMBasicBlockRef {
        self.basic_block_ref
    }
}

#[derive(Clone)]
pub enum Metadata<'ctx> {
    MDNode(MDNode<'ctx>),
    DIDerivedType(DIDerivedType<'ctx>),
    DICompositeType(DICompositeType<'ctx>),
    DISubprogram(DISubprogram<'ctx>),
    Other(LLVMValueRef),
}

impl<'ctx> std::fmt::Debug for Metadata<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self.value_ref();
        let value_string = Message {
            ptr: unsafe { LLVMPrintValueToString(value) },
        }
        .as_c_str()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        let struct_name = match self {
            Self::MDNode(_) => "MDNode",
            Self::DIDerivedType(_) => "DIDerivedType",
            Self::DICompositeType(_) => "DICompositeType",
            Self::DISubprogram(_) => "DISubprogram",
            Self::Other(_) => "Other",
        };
        f.debug_struct(struct_name)
            .field("value", &value_string)
            .finish()
    }
}

impl<'ctx> Operands for Metadata<'ctx> {}

impl<'ctx> ValueRef for Metadata<'ctx> {
    fn value_ref(&self) -> LLVMValueRef {
        match self {
            Self::MDNode(mdnode) => mdnode.value_ref(),
            Self::DIDerivedType(di_derived_type) => di_derived_type.value_ref(),
            Self::DICompositeType(di_composite_type) => di_composite_type.value_ref(),
            Self::DISubprogram(di_subprogram) => di_subprogram.value_ref(),
            Self::Other(value) => *value,
        }
    }
}

impl<'ctx> Metadata<'ctx> {
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

impl<'ctx> TryFrom<DIType<'ctx>> for Metadata<'ctx> {
    type Error = ();

    fn try_from(value: DIType<'ctx>) -> Result<Self, Self::Error> {
        Ok(unsafe { Self::from_value_ref(value.value_ref) })
    }
}

impl<'ctx> TryFrom<DIDerivedType<'ctx>> for Metadata<'ctx> {
    type Error = ();

    fn try_from(value: DIDerivedType<'ctx>) -> Result<Self, Self::Error> {
        Ok(unsafe { Self::from_value_ref(value.value_ref()) })
    }
}

/// Represents a metadata node.
#[derive(Clone)]
pub struct MDNode<'ctx> {
    pub(super) value_ref: LLVMValueRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> std::fmt::Debug for MDNode<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value_str = Message {
            ptr: unsafe { LLVMPrintValueToString(self.value_ref) },
        }
        .as_c_str()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        f.debug_struct("MDNode").field("value", &value_str).finish()
    }
}

impl<'ctx> ValueRef for MDNode<'ctx> {
    fn value_ref(&self) -> LLVMValueRef {
        self.value_ref
    }
}

impl<'ctx> MDNode<'ctx> {
    /// Constructs a new [`MDNode`] from the given `metadata`.
    ///
    /// # Safety
    ///
    /// This method assumes that the given `metadata` corresponds to a valid
    /// instance of [LLVM `MDNode`](https://llvm.org/doxygen/classllvm_1_1MDNode.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any validation checks.
    pub(crate) unsafe fn from_metadata_ref(
        context: &mut Context,
        metadata: LLVMMetadataRef,
    ) -> Self {
        MDNode::from_value_ref(LLVMMetadataAsValue(context.context_ref(), metadata))
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
    pub fn empty(context: &mut Context) -> Self {
        let metadata =
            unsafe { LLVMMDNodeInContext2(context.context_ref(), core::ptr::null_mut(), 0) };
        unsafe { Self::from_metadata_ref(context, metadata) }
    }

    /// Constructs a new metadata node from an array of [`DIType`] elements.
    ///
    /// This function is used to create composite metadata structures, such as
    /// arrays or tuples of different types or values, which can then be used
    /// to represent complex data structures within the metadata system.
    pub fn with_elements(context: &mut Context, elements: &[DIType]) -> Self {
        let metadata = unsafe {
            let mut elements: Vec<LLVMMetadataRef> = elements
                .iter()
                .map(|di_type| LLVMValueAsMetadata(di_type.value_ref))
                .collect();
            LLVMMDNodeInContext2(
                context.context_ref(),
                elements.as_mut_slice().as_mut_ptr(),
                elements.len(),
            )
        };
        unsafe { Self::from_metadata_ref(context, metadata) }
    }
}

pub struct MetadataEntries {
    pub entries: *mut LLVMValueMetadataEntry,
    pub count: usize,
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

#[derive(Clone)]
pub enum User<'ctx> {
    ConstantInt(ConstantInt<'ctx>),
    Function(Function<'ctx>),
    Instruction(Instruction<'ctx>),
}

impl<'ctx> Operands for User<'ctx> {}

impl<'ctx> ValueRef for User<'ctx> {
    fn value_ref(&self) -> LLVMValueRef {
        match self {
            Self::ConstantInt(constant_int) => constant_int.value_ref(),
            Self::Function(f) => f.value_ref(),
            Self::Instruction(inst) => inst.value_ref(),
        }
    }
}

#[derive(Clone)]
pub struct ConstantInt<'ctx> {
    pub value_ref: LLVMValueRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> Operands for ConstantInt<'ctx> {}

impl<'ctx> ValueRef for ConstantInt<'ctx> {
    fn value_ref(&self) -> LLVMValueRef {
        self.value_ref
    }
}

impl<'ctx> ConstantInt<'ctx> {
    pub(crate) unsafe fn from_value_ref(value_ref: LLVMValueRef) -> Self {
        Self {
            value_ref,
            _marker: PhantomData,
        }
    }

    pub fn new_32(value: u32) -> Self {
        unsafe {
            let value_ref = LLVMConstInt(LLVMInt32Type(), value as u64, 0);
            Self::from_value_ref(value_ref)
        }
    }

    /// Returns the constant as a `u64`.
    pub fn unsigned(&self) -> u64 {
        unsafe { LLVMConstIntGetZExtValue(self.value_ref) }
    }
}

/// Represents a function.
#[derive(Clone)]
pub struct Function<'ctx> {
    pub value_ref: LLVMValueRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> Operands for Function<'ctx> {}

impl<'ctx> ValueRef for Function<'ctx> {
    fn value_ref(&self) -> LLVMValueRef {
        self.value_ref
    }
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

    pub fn intrinsic_declaration(module: &mut Module, id: u32, param_types: &[Type]) -> Self {
        let mut param_types: Vec<LLVMTypeRef> =
            param_types.into_iter().map(|t| t.type_ref).collect();
        unsafe {
            let value_ref = LLVMGetIntrinsicDeclaration(
                module.module_ref(),
                id,
                param_types.as_mut_ptr(),
                param_types.len(),
            );
            Self::from_value_ref(value_ref)
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

    pub(crate) fn subprogram(&self, context: &mut Context) -> Option<DISubprogram<'ctx>> {
        let subprogram = unsafe { LLVMGetSubprogram(self.value_ref) };
        NonNull::new(subprogram).map(|_| unsafe {
            DISubprogram::from_value_ref(LLVMMetadataAsValue(context.context_ref(), subprogram))
        })
    }

    pub(crate) fn set_subprogram(&mut self, subprogram: &DISubprogram) {
        unsafe { LLVMSetSubprogram(self.value_ref, LLVMValueAsMetadata(subprogram.value_ref)) };
    }

    pub fn function_type(&self) -> Type {
        unsafe {
            let type_ref = LLVMGlobalGetValueType(self.value_ref());
            Type::from_type_ref(type_ref)
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum Instruction<'ctx> {
    DbgVariableIntrinsic(DbgVariableIntrinsic<'ctx>),
    GetElementPtrInst(GetElementPtrInst<'ctx>),
    IntToPtrInst(IntToPtrInst<'ctx>),
    LoadInst(LoadInst<'ctx>),
    Other(LLVMValueRef),
}

impl<'ctx> Instruction<'ctx> {
    pub fn new(value: LLVMValueRef) -> Self {
        if unsafe { !LLVMIsADbgVariableIntrinsic(value).is_null() } {
            let dbg_variable_intrinsic = unsafe { DbgVariableIntrinsic::from_value_ref(value) };
            return Instruction::DbgVariableIntrinsic(dbg_variable_intrinsic);
        }
        if unsafe { !LLVMIsAGetElementPtrInst(value).is_null() } {
            let get_element_ptr_inst = unsafe { GetElementPtrInst::from_value_ref(value) };
            return Instruction::GetElementPtrInst(get_element_ptr_inst);
        }
        if unsafe { !LLVMIsAIntToPtrInst(value).is_null() } {
            let int_to_ptr_inst = unsafe { IntToPtrInst::from_value_ref(value) };
            return Instruction::IntToPtrInst(int_to_ptr_inst);
        }
        if unsafe { !LLVMIsALoadInst(value).is_null() } {
            let load_inst = unsafe { LoadInst::from_value_ref(value) };
            return Instruction::LoadInst(load_inst);
        }
        Instruction::Other(value)
    }

    // pub fn dbg_records(&self) -> impl Iterator<Item = DbgRecord> + '_ {
    //     self.value_ref()
    //         .dbg_records_iter()
    //         .map(|dbg_record_ref| DbgRecord::from_dbg_record_ref(dbg_record_ref))
    // }

    pub fn has_metadata(&self) -> bool {
        unsafe { LLVMHasMetadata(self.value_ref()) > 0 }
    }

    pub fn next_instruction(&self) -> Self {
        let value = unsafe { LLVMGetNextInstruction(self.value_ref()) };
        Self::new(value)
    }
}

impl<'ctx> From<IntToPtrInst<'ctx>> for Instruction<'ctx> {
    fn from(value: IntToPtrInst<'ctx>) -> Self {
        Self::IntToPtrInst(value)
    }
}

impl<'ctx> From<GetElementPtrInst<'ctx>> for Instruction<'ctx> {
    fn from(value: GetElementPtrInst<'ctx>) -> Self {
        Self::GetElementPtrInst(value)
    }
}

impl<'ctx> std::fmt::Debug for Instruction<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value_s = Message {
            ptr: unsafe { LLVMPrintValueToString(self.value_ref()) },
        }
        .as_c_str()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        match self {
            Self::DbgVariableIntrinsic(_) => f
                .debug_struct("DbgVariableIntrinsic")
                .field("value", &value_s)
                .finish(),
            Self::GetElementPtrInst(_) => {
                f.debug_struct("Function").field("value", &value_s).finish()
            }
            Self::IntToPtrInst(_) => f
                .debug_struct("IntToPtrInst")
                .field("value", &value_s)
                .finish(),
            Self::LoadInst(_) => f.debug_struct("LoadInst").field("value", &value_s).finish(),
            Self::Other(_) => f.debug_struct("Other").field("value", &value_s).finish(),
        }
    }
}

impl<'ctx> Operands for Instruction<'ctx> {}

impl<'ctx> ValueRef for Instruction<'ctx> {
    fn value_ref(&self) -> LLVMValueRef {
        match self {
            Self::DbgVariableIntrinsic(dbg_variable_intrinsic) => dbg_variable_intrinsic.value_ref,
            Self::GetElementPtrInst(get_element_ptr_inst) => get_element_ptr_inst.value_ref,
            Self::IntToPtrInst(int_to_ptr_inst) => int_to_ptr_inst.value_ref,
            Self::LoadInst(load_inst) => load_inst.value_ref,
            Self::Other(value_ref) => *value_ref,
        }
    }
}

#[repr(u32)]
enum DbgVariableIntrinsicOperand {
    Variable = 1,
}

#[derive(Clone, Eq, PartialEq)]
pub struct DbgVariableIntrinsic<'ctx> {
    pub value_ref: LLVMValueRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> std::fmt::Debug for DbgVariableIntrinsic<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value_str = Message {
            ptr: unsafe { LLVMPrintValueToString(self.value_ref) },
        }
        .as_c_str()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        f.debug_struct("DbgValueInst")
            .field("value", &value_str)
            .finish()
    }
}

impl<'ctx> DbgVariableIntrinsic<'ctx> {
    pub(crate) unsafe fn from_value_ref(value_ref: LLVMValueRef) -> Self {
        Self {
            value_ref,
            _marker: PhantomData,
        }
    }

    pub fn value_ref(&self) -> LLVMValueRef {
        self.value_ref
    }

    pub fn variable(&self) -> DILocalVariable<'_> {
        let value_ref =
            unsafe { LLVMGetOperand(self.value_ref, DbgVariableIntrinsicOperand::Variable as u32) };
        unsafe { DILocalVariable::from_value_ref(value_ref) }
    }
}

#[repr(u32)]
enum GetElementPtrInstOperand {
    Pointer = 0,
}

#[derive(Clone, Eq, PartialEq)]
pub struct GetElementPtrInst<'ctx> {
    pub value_ref: LLVMValueRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> std::fmt::Debug for GetElementPtrInst<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value_str = Message {
            ptr: unsafe { LLVMPrintValueToString(self.value_ref) },
        }
        .as_c_str()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        f.debug_struct("GetElementPtrInst")
            .field("value", &value_str)
            .finish()
    }
}

impl<'ctx> Operands for GetElementPtrInst<'ctx> {}

impl<'ctx> ValueRef for GetElementPtrInst<'ctx> {
    fn value_ref(&self) -> LLVMValueRef {
        self.value_ref
    }
}

impl<'ctx> GetElementPtrInst<'ctx> {
    pub(crate) unsafe fn from_value_ref(value_ref: LLVMValueRef) -> Self {
        Self {
            value_ref,
            _marker: PhantomData,
        }
    }

    pub fn indices(&self) -> OperandsIterator {
        OperandsIterator::new_from(self.value_ref(), 1)
    }

    pub fn pointer_operand(&self) -> Value {
        let operand =
            unsafe { LLVMGetOperand(self.value_ref, GetElementPtrInstOperand::Pointer as u32) };
        Value::new(operand)
    }

    pub fn source_element_type(&self) -> Type {
        unsafe {
            let type_ref = LLVMGetGEPSourceElementType(self.value_ref());
            Type::from_type_ref(type_ref)
        }
    }

    pub fn value_ref(&self) -> LLVMValueRef {
        self.value_ref
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct IntToPtrInst<'ctx> {
    pub value_ref: LLVMValueRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> std::fmt::Debug for IntToPtrInst<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value_str = Message {
            ptr: unsafe { LLVMPrintValueToString(self.value_ref) },
        }
        .as_c_str()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        f.debug_struct("IntToPtrInst")
            .field("value", &value_str)
            .finish()
    }
}

impl<'ctx> IntToPtrInst<'ctx> {
    pub(crate) unsafe fn from_value_ref(value_ref: LLVMValueRef) -> Self {
        Self {
            value_ref,
            _marker: PhantomData,
        }
    }

    pub fn value_ref(&self) -> LLVMValueRef {
        self.value_ref
    }
}

#[repr(u32)]
enum LoadInstOperand {
    Pointer = 0,
}

#[derive(Clone, Eq, PartialEq)]
pub struct LoadInst<'ctx> {
    pub value_ref: LLVMValueRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> std::fmt::Debug for LoadInst<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value_str = Message {
            ptr: unsafe { LLVMPrintValueToString(self.value_ref) },
        }
        .as_c_str()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        f.debug_struct("LoadInst")
            .field("value", &value_str)
            .finish()
    }
}

impl<'ctx> LoadInst<'ctx> {
    pub(crate) unsafe fn from_value_ref(value_ref: LLVMValueRef) -> Self {
        Self {
            value_ref,
            _marker: PhantomData,
        }
    }

    pub fn pointer_operand(&self) -> Value {
        let operand = unsafe { LLVMGetOperand(self.value_ref, LoadInstOperand::Pointer as u32) };
        Value::new(operand)
    }

    pub fn value_ref(&self) -> LLVMValueRef {
        self.value_ref
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Type<'ctx> {
    pub type_ref: LLVMTypeRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> std::fmt::Debug for Type<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let type_string = Message {
            ptr: unsafe { LLVMPrintTypeToString(self.type_ref) },
        }
        .as_c_str()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        f.debug_struct("Type").field("type", &type_string).finish()
    }
}

impl<'ctx> Type<'ctx> {
    pub(crate) unsafe fn from_type_ref(type_ref: LLVMTypeRef) -> Self {
        Self {
            type_ref,
            _marker: PhantomData,
        }
    }

    pub fn int8(context: &mut Context) -> Self {
        unsafe {
            let type_ref = LLVMInt8TypeInContext(context.context_ref());
            Type::from_type_ref(type_ref)
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct DbgRecord<'ctx> {
    pub dbg_record_ref: LLVMDbgRecordRef,
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> std::fmt::Debug for DbgRecord<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dbg_record_string = Message {
            ptr: unsafe { LLVMPrintDbgRecordToString(self.dbg_record_ref) },
        }
        .as_c_str()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        f.debug_struct("DbgRecord")
            .field("dbg_record", &dbg_record_string)
            .finish()
    }
}

impl<'ctx> DbgRecord<'ctx> {
    pub(crate) fn from_dbg_record_ref(dbg_record_ref: LLVMDbgRecordRef) -> Self {
        Self {
            dbg_record_ref,
            _marker: PhantomData,
        }
    }
}
