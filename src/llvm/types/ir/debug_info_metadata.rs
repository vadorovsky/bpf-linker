use std::{
    ffi::{c_uchar, CStr, NulError},
    ptr::NonNull,
    slice, str,
};

use gimli::DwTag;
use llvm_sys::{
    core::{
        LLVMGetMDString, LLVMGetNumOperands, LLVMGetOperand, LLVMGetTypeContext,
        LLVMMetadataAsValue, LLVMReplaceMDNodeOperandWith, LLVMTypeOf, LLVMValueAsMetadata,
    },
    debuginfo::{
        LLVMDIFileGetFilename, LLVMDIFlags, LLVMDIScopeGetFile, LLVMDISubprogramGetLine,
        LLVMDITypeGetFlags, LLVMDITypeGetLine, LLVMDITypeGetName, LLVMDITypeGetOffsetInBits,
        LLVMGetDINodeTag, LLVMGetMetadataKind, LLVMMetadataKind,
    },
    prelude::{LLVMMetadataRef, LLVMValueRef},
    LLVMContext, LLVMOpaqueMetadata, LLVMValue,
};

use crate::llvm::types::{
    ir::{LLVMTypeWrapperWithContext, MDNode, Metadata},
    LLVMMetadataWrapper, LLVMTypeError, LLVMTypeWrapper,
};

/// Returns a DWARF tag for the given debug info node.
///
/// This function should be called in `tag` method of all LLVM debug info types
/// inheriting from [`DINode`](https://llvm.org/doxygen/classllvm_1_1DINode.html).
///
/// # Safety
///
/// This function assumes that the given `metadata_ref` corresponds to a valid
/// instance of [LLVM `DINode`](https://llvm.org/doxygen/classllvm_1_1DINode.html).
/// It's the caller's responsibility to ensure this invariant, as this function
/// doesn't perform any validation checks.
unsafe fn di_node_tag(metadata: NonNull<LLVMOpaqueMetadata>) -> DwTag {
    DwTag(LLVMGetDINodeTag(metadata.as_ptr()))
}

pub struct DIScope {
    metadata: NonNull<LLVMOpaqueMetadata>,
    value: NonNull<LLVMValue>,
}

impl LLVMTypeWrapper for DIScope {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError>
    where
        Self: Sized,
    {
        let metadata = unsafe { LLVMValueAsMetadata(value.as_ptr()) };
        let metadata = NonNull::new(metadata).ok_or(LLVMTypeError::NullPointer)?;
        let metadata_kind = unsafe { LLVMGetMetadataKind(metadata.as_ptr()) };
        // The children of `DIScope` are:
        //
        // - `DICommonBlock`
        // - `DICompileUnit`
        // - `DIFile`
        // - `DILocalScope`
        //   - `DILexicalBlockBase`
        //     - `DILexicalBlock`
        //     - `DILexicalBlockFile`
        //   - `DISubprogram`
        // - `DIModule`
        // - `DINamespace`
        // - `DIType`
        //   - `DIBasicType`
        //   - `DICompositeType`
        //   - `DIDerivedType`
        //   - `DIStringType`
        //   - `DISubroutimeType`
        //
        // Parent classes do not have coresponding `LLVMMetadataKind` enum
        // variants, only the last children do.
        //
        // https://llvm.org/doxygen/classllvm_1_1DIScope.html
        // https://llvm.org/doxygen/classllvm_1_1DILocalScope.html
        // https://llvm.org/doxygen/classllvm_1_1DIType.html
        match metadata_kind {
            LLVMMetadataKind::LLVMDICommonBlockMetadataKind
            | LLVMMetadataKind::LLVMDICompileUnitMetadataKind
            | LLVMMetadataKind::LLVMDIFileMetadataKind
            | LLVMMetadataKind::LLVMDILexicalBlockMetadataKind
            | LLVMMetadataKind::LLVMDILexicalBlockFileMetadataKind
            | LLVMMetadataKind::LLVMDISubprogramMetadataKind
            | LLVMMetadataKind::LLVMDIModuleMetadataKind
            | LLVMMetadataKind::LLVMDINamespaceMetadataKind
            | LLVMMetadataKind::LLVMDIBasicTypeMetadataKind
            | LLVMMetadataKind::LLVMDICompositeTypeMetadataKind
            | LLVMMetadataKind::LLVMDIDerivedTypeMetadataKind
            | LLVMMetadataKind::LLVMDIStringTypeMetadataKind
            | LLVMMetadataKind::LLVMDISubroutineTypeMetadataKind => Ok(Self { metadata, value }),
            _ => Err(LLVMTypeError::InvalidPointerType("DIScope")),
        }
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.value
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.value.as_ptr()
    }
}

impl DIScope {
    pub fn as_metadata_ptr(&self) -> LLVMMetadataRef {
        self.metadata.as_ptr()
    }
}

/// Represents a source code file in debug infomation.
///
/// A `DIFile` debug info node, which represents a given file, is referenced by
/// other debug info nodes which belong to the file.
pub struct DIFile {
    metadata: NonNull<LLVMOpaqueMetadata>,
}

impl LLVMTypeWrapper for DIFile {
    type Target = LLVMOpaqueMetadata;

    fn from_ptr(metadata: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        let metadata_kind = unsafe { LLVMGetMetadataKind(metadata.as_ptr()) };
        if !matches!(metadata_kind, LLVMMetadataKind::LLVMDIFileMetadataKind) {
            return Err(LLVMTypeError::InvalidPointerType("DIFile"));
        }
        Ok(Self { metadata })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.metadata
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.metadata.as_ptr()
    }
}

impl DIFile {
    pub fn filename(&self) -> Option<&CStr> {
        let mut len = 0;
        // `LLVMDIFileGetName` doesn't allocate any memory, it just returns
        // a pointer to the string which is already a part of `DIFile`:
        // https://github.com/llvm/llvm-project/blob/eee1f7cef856241ad7d66b715c584d29b1c89ca9/llvm/lib/IR/DebugInfo.cpp#L1175-L1179
        //
        // Therefore, we don't need to call `LLVMDisposeMessage`. The memory
        // gets freed when calling `LLVMDisposeDIBuilder`.
        let ptr = unsafe { LLVMDIFileGetFilename(self.metadata.as_ptr(), &mut len) };
        NonNull::new(ptr as *mut _).map(|ptr| unsafe { CStr::from_ptr(ptr.as_ptr()) })
    }
}

/// Represents the operands for a [`DIType`]. The enum values correspond to the
/// operand indices within metadata nodes.
#[repr(u32)]
enum DITypeOperand {
    /// Name of the type.
    /// [Reference in LLVM code](https://github.com/llvm/llvm-project/blob/llvmorg-17.0.3/llvm/include/llvm/IR/DebugInfoMetadata.h#L743).
    Name = 2,
}

/// Returns the name of the type.
///
/// This function should be called in `name` method of `DIType` and all other
/// LLVM debug info types inheriting from it.
///
/// # Safety
///
/// This function assumes that the given `metadata_ref` corresponds to a valid
/// instance of [LLVM `DIType`](https://llvm.org/doxygen/classllvm_1_1DIType.html).
/// It's the caller's responsibility to ensure this invariant, as this function
/// doesn't perform any validation checks.
unsafe fn di_type_name<'a>(metadata: NonNull<LLVMOpaqueMetadata>) -> Option<&'a CStr> {
    let mut len = 0;
    // `LLVMDITypeGetName` doesn't allocate any memory, it just returns
    // a pointer to the string which is already a part of `DIType`:
    // https://github.com/llvm/llvm-project/blob/eee1f7cef856241ad7d66b715c584d29b1c89ca9/llvm/lib/IR/DebugInfo.cpp#L1489-L1493
    //
    // Therefore, we don't need to call `LLVMDisposeMessage`. The memory
    // gets freed when calling `LLVMDisposeDIBuilder`. Example:
    // https://github.com/llvm/llvm-project/blob/eee1f7cef856241ad7d66b715c584d29b1c89ca9/llvm/tools/llvm-c-test/debuginfo.c#L249-L255
    let ptr = LLVMDITypeGetName(metadata.as_ptr(), &mut len);
    NonNull::new(ptr as *mut _).map(|ptr| CStr::from_ptr(ptr.as_ptr()))
}

/// Represents the debug information for a primitive type in LLVM IR.
pub struct DIType {
    metadata: NonNull<LLVMOpaqueMetadata>,
    value: NonNull<LLVMValue>,
}

impl LLVMTypeWrapper for DIType {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        let metadata = unsafe { LLVMValueAsMetadata(value.as_ptr()) };
        let metadata = NonNull::new(metadata).ok_or(LLVMTypeError::NullPointer)?;
        let metadata_kind = unsafe { LLVMGetMetadataKind(metadata.as_ptr()) };
        // The children of `DIType` are:
        //
        // - `DIBasicType`
        // - `DICompositeType`
        // - `DIDerivedType`
        // - `DIStringType`
        // - `DISubroutimeType`
        //
        // https://llvm.org/doxygen/classllvm_1_1DIType.html
        match metadata_kind {
            LLVMMetadataKind::LLVMDIBasicTypeMetadataKind
            | LLVMMetadataKind::LLVMDICompositeTypeMetadataKind
            | LLVMMetadataKind::LLVMDIDerivedTypeMetadataKind
            | LLVMMetadataKind::LLVMDIStringTypeMetadataKind
            | LLVMMetadataKind::LLVMDISubroutineTypeMetadataKind => Ok(Self { metadata, value }),
            _ => Err(LLVMTypeError::InvalidPointerType("DIType")),
        }
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.value
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.value.as_ptr()
    }
}

impl DIType {
    /// Returns the offset of the type in bits. This offset is used in case the
    /// type is a member of a composite type.
    pub fn offset_in_bits(&self) -> usize {
        unsafe { LLVMDITypeGetOffsetInBits(self.metadata.as_ptr()) as usize }
    }
}

impl From<DIDerivedType> for DIType {
    fn from(di_derived_type: DIDerivedType) -> Self {
        Self::from_ptr(di_derived_type.value).unwrap()
    }
}

/// Represents the operands for a [`DIDerivedType`]. The enum values correspond
/// to the operand indices within metadata nodes.
#[repr(u32)]
enum DIDerivedTypeOperand {
    /// [`DIType`] representing a base type of the given derived type.
    /// [Reference in LLVM code](https://github.com/llvm/llvm-project/blob/llvmorg-17.0.3/llvm/include/llvm/IR/DebugInfoMetadata.h#L1032).
    BaseType = 3,
}

/// Represents the debug information for a derived type in LLVM IR.
///
/// The types derived from other types usually add a level of indirection or an
/// alternative name. The examples of derived types are pointers, references,
/// typedefs, etc.
pub struct DIDerivedType {
    metadata: NonNull<LLVMOpaqueMetadata>,
    value: NonNull<LLVMValue>,
}

impl LLVMTypeWrapper for DIDerivedType {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        let metadata = unsafe { LLVMValueAsMetadata(value.as_ptr()) };
        let metadata = NonNull::new(metadata).ok_or(LLVMTypeError::NullPointer)?;
        let metadata_kind = unsafe { LLVMGetMetadataKind(metadata.as_ptr()) };
        if !matches!(
            metadata_kind,
            LLVMMetadataKind::LLVMDIDerivedTypeMetadataKind,
        ) {
            return Err(LLVMTypeError::InvalidPointerType("DIDerivedType"));
        }
        Ok(Self { metadata, value })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.value
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.value.as_ptr()
    }
}

impl DIDerivedType {
    /// Returns the base type of this derived type.
    pub fn base_type(&self) -> Metadata {
        unsafe {
            let value = LLVMGetOperand(self.value.as_ptr(), DIDerivedTypeOperand::BaseType as u32);
            let value = NonNull::new(value).expect("base type operand should not be null");
            Metadata::from_value(value)
                .expect("base type pointer should be an instance of Metadata")
        }
    }

    /// Replaces the name of the type with a new name.
    ///
    /// # Errors
    ///
    /// Returns a `NulError` if the new name contains a NUL byte, as it cannot
    /// be converted into a `CString`.
    pub fn replace_name(&mut self, name: &str) -> Result<(), NulError> {
        let context = unsafe { LLVMGetTypeContext(LLVMTypeOf(self.as_ptr())) };
        super::replace_name(
            self.value.as_ptr(),
            context,
            DITypeOperand::Name as u32,
            name,
        )
    }

    /// Returns a DWARF tag of the given derived type.
    pub fn tag(&self) -> DwTag {
        unsafe { di_node_tag(self.metadata) }
    }
}

/// Represents the operands for a [`DICompositeType`]. The enum values
/// correspond to the operand indices within metadata nodes.
#[repr(u32)]
enum DICompositeTypeOperand {
    /// Elements of the composite type.
    /// [Reference in LLVM code](https://github.com/llvm/llvm-project/blob/llvmorg-17.0.3/llvm/include/llvm/IR/DebugInfoMetadata.h#L1230).
    Elements = 4,
}

/// Represents the debug info for a composite type in LLVM IR.
///
/// Composite type is a kind of type that can include other types, such as
/// structures, enums, unions, etc.
pub struct DICompositeType {
    metadata: NonNull<LLVMOpaqueMetadata>,
    value: NonNull<LLVMValue>,
}

impl LLVMTypeWrapper for DICompositeType {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        let metadata = unsafe { LLVMValueAsMetadata(value.as_ptr()) };
        let metadata = NonNull::new(metadata).expect("metadata pointer should not be null");
        let metadata_kind = unsafe { LLVMGetMetadataKind(metadata.as_ptr()) };
        if !matches!(
            metadata_kind,
            LLVMMetadataKind::LLVMDICompositeTypeMetadataKind,
        ) {
            return Err(LLVMTypeError::InvalidPointerType("DICompositeType"));
        }
        Ok(Self { metadata, value })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.value
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.value.as_ptr()
    }
}

impl LLVMTypeWrapperWithContext for DICompositeType {}

impl DICompositeType {
    /// Returns an iterator over elements (struct fields, enum variants, etc.)
    /// of the composite type.
    pub fn elements(&self) -> impl Iterator<Item = Metadata> {
        let elements =
            unsafe { LLVMGetOperand(self.value.as_ptr(), DICompositeTypeOperand::Elements as u32) };
        let operands = NonNull::new(elements)
            .map(|elements| unsafe { LLVMGetNumOperands(elements.as_ptr()) })
            .unwrap_or(0);

        (0..operands).map(move |i| {
            let operand = unsafe { LLVMGetOperand(elements, i as u32) };
            let operand = NonNull::new(operand).expect("element operand should not be null");
            Metadata::from_value(operand).expect("operands should be instances of Metadata")
        })
    }

    /// Returns the name of the composite type.
    pub fn name(&self) -> Option<&CStr> {
        unsafe { di_type_name(self.metadata) }
    }

    /// Returns the file that the composite type belongs to.
    pub fn file(&self) -> DIFile {
        unsafe {
            let metadata = LLVMDIScopeGetFile(self.metadata.as_ptr());
            let metadata = NonNull::new(metadata).expect("metadata pointer should not be null");
            DIFile::from_ptr(metadata).expect("the pointer should be of type DIFile")
        }
    }

    /// Returns the flags associated with the composity type.
    pub fn flags(&self) -> LLVMDIFlags {
        unsafe { LLVMDITypeGetFlags(self.metadata.as_ptr()) }
    }

    /// Returns the line number in the source code where the type is defined.
    pub fn line(&self) -> u32 {
        unsafe { LLVMDITypeGetLine(self.metadata.as_ptr()) }
    }

    /// Replaces the elements of the composite type with a new metadata node.
    /// The provided metadata node should contain new composite type elements
    /// as operants. The metadata node can be empty if the intention is to
    /// remove all elements of the composite type.
    pub fn replace_elements(&mut self, mdnode: MDNode) {
        unsafe {
            LLVMReplaceMDNodeOperandWith(
                self.value.as_ptr(),
                DICompositeTypeOperand::Elements as u32,
                LLVMValueAsMetadata(mdnode.as_ptr()),
            )
        }
    }

    /// Replaces the name of the type with a new name.
    ///
    /// # Errors
    ///
    /// Returns a `NulError` if the new name contains a NUL byte, as it cannot
    /// be converted into a `CString`.
    pub fn replace_name(&mut self, name: &str) -> Result<(), NulError> {
        let context = unsafe { LLVMGetTypeContext(LLVMTypeOf(self.as_ptr())) };
        super::replace_name(self.as_ptr(), context, DITypeOperand::Name as u32, name)
    }

    /// Returns a DWARF tag of the given composite type.
    pub fn tag(&self) -> DwTag {
        unsafe { di_node_tag(self.metadata) }
    }
}

/// Type array for a subprogram.
pub struct DISubroutineType {
    metadata: NonNull<LLVMOpaqueMetadata>,
    value: NonNull<LLVMValue>,
}

impl LLVMTypeWrapper for DISubroutineType {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        let metadata = unsafe { LLVMValueAsMetadata(value.as_ptr()) };
        let metadata = NonNull::new(metadata).expect("metadata pointer should not be null");
        let metadata_kind = unsafe { LLVMGetMetadataKind(metadata.as_ptr()) };
        if !matches!(
            metadata_kind,
            LLVMMetadataKind::LLVMDISubroutineTypeMetadataKind,
        ) {
            return Err(LLVMTypeError::InvalidPointerType("DISubroutineType"));
        }
        Ok(Self { metadata, value })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.value
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.value.as_ptr()
    }
}

impl DISubroutineType {
    pub fn as_metadata_ptr(&self) -> LLVMMetadataRef {
        self.metadata.as_ptr()
    }
}

/// Represents the operands for a [`DISubprogram`]. The enum values correspond
/// to the operand indices within metadata nodes.
#[repr(u32)]
enum DISubprogramOperand {
    Scope = 1,
    Name = 2,
    LinkageName = 3,
    Ty = 4,
    Unit = 5,
    RetainedNodes = 7,
}

fn mdstring_to_str<'a>(mdstring: LLVMValueRef) -> &'a str {
    let mut len = 0;
    let ptr = unsafe { LLVMGetMDString(mdstring, &mut len) };
    unsafe { str::from_utf8(slice::from_raw_parts(ptr as *const c_uchar, len as usize)).unwrap() }
}

/// Represents the debug information for a subprogram (function) in LLVM IR.
pub struct DISubprogram {
    metadata: NonNull<LLVMOpaqueMetadata>,
    value: NonNull<LLVMValue>,
}

impl LLVMMetadataWrapper for DISubprogram {
    fn from_metadata_ptr(
        metadata: NonNull<LLVMOpaqueMetadata>,
        context: NonNull<LLVMContext>,
    ) -> Result<Self, LLVMTypeError>
    where
        Self: Sized,
    {
        let metadata_kind = unsafe { LLVMGetMetadataKind(metadata.as_ptr()) };
        if !matches!(
            metadata_kind,
            LLVMMetadataKind::LLVMDISubprogramMetadataKind,
        ) {
            return Err(LLVMTypeError::InvalidPointerType("DISubprogram"));
        }
        let value = unsafe { LLVMMetadataAsValue(context.as_ptr(), metadata.as_ptr()) };
        let value = NonNull::new(value).unwrap();
        Self::from_ptr(value)
    }

    fn as_metadata_ptr(&self) -> LLVMMetadataRef {
        self.metadata.as_ptr()
    }
}

impl LLVMTypeWrapper for DISubprogram {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        let metadata = unsafe { LLVMValueAsMetadata(value.as_ptr()) };
        let metadata = NonNull::new(metadata).expect("metadata should not be null");
        let metadata_kind = unsafe { LLVMGetMetadataKind(metadata.as_ptr()) };
        if !matches!(
            metadata_kind,
            LLVMMetadataKind::LLVMDISubprogramMetadataKind,
        ) {
            return Err(LLVMTypeError::InvalidPointerType("DISubprogram"));
        }
        Ok(DISubprogram { metadata, value })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.value
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.value.as_ptr()
    }
}

impl DISubprogram {
    /// Returns the name of the subprogram.
    pub fn name(&self) -> Option<&str> {
        let operand =
            unsafe { LLVMGetOperand(self.value.as_ptr(), DISubprogramOperand::Name as u32) };
        NonNull::new(operand).map(|_| mdstring_to_str(operand))
    }

    /// Returns the linkage name of the subprogram.
    pub fn linkage_name(&self) -> Option<&str> {
        let operand =
            unsafe { LLVMGetOperand(self.value.as_ptr(), DISubprogramOperand::LinkageName as u32) };
        NonNull::new(operand).map(|_| mdstring_to_str(operand))
    }

    pub fn ty(&self) -> DISubroutineType {
        let ty = unsafe { LLVMGetOperand(self.value.as_ptr(), DISubprogramOperand::Ty as u32) };
        let ty = NonNull::new(ty).expect("subprogram's type should not be null");
        DISubroutineType::from_ptr(ty)
            .expect("subprogram's type should be a valid DISubroutineType pointer")
    }

    pub fn file(&self) -> DIFile {
        let file = unsafe { LLVMDIScopeGetFile(LLVMValueAsMetadata(self.value.as_ptr())) };
        let file = NonNull::new(file).expect("subprogram's file should not be null");
        DIFile::from_ptr(file).expect("subprogram's file should be a valid DIFile pointer")
    }

    pub fn line(&self) -> u32 {
        unsafe { LLVMDISubprogramGetLine(LLVMValueAsMetadata(self.value.as_ptr())) }
    }

    pub fn type_flags(&self) -> i32 {
        unsafe { LLVMDITypeGetFlags(LLVMValueAsMetadata(self.value.as_ptr())) }
    }

    /// Replaces the name of the subprogram with a new name.
    ///
    /// # Errors
    ///
    /// Returns a `NulError` if the new name contains a NUL byte, as it cannot
    /// be converted into a `CString`.
    pub fn replace_name(&mut self, name: &str) -> Result<(), NulError> {
        let context = unsafe { LLVMGetTypeContext(LLVMTypeOf(self.as_ptr())) };
        super::replace_name(
            self.as_ptr(),
            context,
            DISubprogramOperand::Name as u32,
            name,
        )
    }

    pub fn scope(&self) -> Option<DIScope> {
        unsafe {
            let operand = LLVMGetOperand(self.value.as_ptr(), DISubprogramOperand::Scope as u32);
            NonNull::new(operand)
                .map(|scope| DIScope::from_ptr(scope).expect("scope should be a valid pointer"))
        }
    }

    pub fn unit(&self) -> Option<LLVMMetadataRef> {
        unsafe {
            let operand = LLVMGetOperand(self.value.as_ptr(), DISubprogramOperand::Unit as u32);
            NonNull::new(operand).map(|_| LLVMValueAsMetadata(operand))
        }
    }

    pub fn set_unit(&mut self, unit: LLVMMetadataRef) {
        unsafe {
            LLVMReplaceMDNodeOperandWith(
                self.value.as_ptr(),
                DISubprogramOperand::Unit as u32,
                unit,
            )
        };
    }

    pub fn retained_nodes(&self) -> Option<LLVMMetadataRef> {
        unsafe {
            let nodes = LLVMGetOperand(
                self.value.as_ptr(),
                DISubprogramOperand::RetainedNodes as u32,
            );
            NonNull::new(nodes).map(|_| LLVMValueAsMetadata(nodes))
        }
    }

    pub fn set_retained_nodes(&mut self, nodes: LLVMMetadataRef) {
        unsafe {
            LLVMReplaceMDNodeOperandWith(
                self.value.as_ptr(),
                DISubprogramOperand::RetainedNodes as u32,
                nodes,
            )
        };
    }
}
