use std::ptr::NonNull;

use llvm_sys::{
    core::{
        LLVMDisposeValueMetadataEntries, LLVMGlobalCopyAllMetadata, LLVMIsAGlobalObject,
        LLVMIsAInstruction, LLVMIsAMDNode, LLVMMetadataAsValue, LLVMValueAsMetadata,
        LLVMValueMetadataEntriesGetKind, LLVMValueMetadataEntriesGetMetadata,
    },
    debuginfo::{LLVMGetMetadataKind, LLVMMetadataKind},
    prelude::LLVMMetadataRef,
    LLVMContext, LLVMOpaqueMetadata, LLVMOpaqueValueMetadataEntry, LLVMValue,
};

use crate::llvm::types::{
    ir::{context::LLVMTypeWrapperWithContext, DICompositeType, DIDerivedType, DISubprogram},
    LLVMMetadataWrapper, LLVMTypeError, LLVMTypeWrapper,
};

/// Root of the metadata hierarchy.
///
/// This is a root class for typeless data in the IR.
pub enum Metadata {
    DICompositeType(DICompositeType),
    DIDerivedType(DIDerivedType),
    DISubprogram(DISubprogram),
    Other(#[allow(dead_code)] NonNull<LLVMValue>),
}

impl Metadata {
    /// Constructs a new [`Metadata`] from the given `value`.
    ///
    /// # Safety
    ///
    /// This method assumes that the provided `value` corresponds to a valid
    /// instance of [LLVM `Metadata`](https://llvm.org/doxygen/classllvm_1_1Metadata.html).
    /// It's the caller's responsibility to ensure this invariant, as this
    /// method doesn't perform any valiation checks.
    pub(crate) fn from_value(value: NonNull<LLVMValue>) -> Result<Self, LLVMTypeError> {
        let metadata = unsafe { LLVMValueAsMetadata(value.as_ptr()) };

        match unsafe { LLVMGetMetadataKind(metadata) } {
            LLVMMetadataKind::LLVMDICompositeTypeMetadataKind => {
                let di_composite_type = DICompositeType::from_ptr(value)?;
                Ok(Metadata::DICompositeType(di_composite_type))
            }
            LLVMMetadataKind::LLVMDIDerivedTypeMetadataKind => {
                let di_derived_type = DIDerivedType::from_ptr(value)?;
                Ok(Metadata::DIDerivedType(di_derived_type))
            }
            LLVMMetadataKind::LLVMDISubprogramMetadataKind => {
                let di_subprogram = DISubprogram::from_ptr(value)?;
                Ok(Metadata::DISubprogram(di_subprogram))
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
            | LLVMMetadataKind::LLVMDIAssignIDMetadataKind => Ok(Metadata::Other(value)),
        }
    }
}

impl TryFrom<MDNode> for Metadata {
    type Error = LLVMTypeError;

    fn try_from(md_node: MDNode) -> Result<Self, Self::Error> {
        // FIXME: fail if md_node isn't a Metadata node
        Self::from_value(md_node.value)
    }
}

/// Represents a metadata node.
#[derive(Clone)]
pub struct MDNode {
    metadata: NonNull<LLVMOpaqueMetadata>,
    value: NonNull<LLVMValue>,
}

impl LLVMMetadataWrapper for MDNode {
    fn from_metadata_ptr(
        metadata: NonNull<LLVMOpaqueMetadata>,
        context: NonNull<LLVMContext>,
    ) -> Result<Self, LLVMTypeError>
    where
        Self: Sized,
    {
        let value = unsafe { LLVMMetadataAsValue(context.as_ptr(), metadata.as_ptr()) };
        let value = NonNull::new(value).expect("value of a non-null metadata should not be null");
        if unsafe { LLVMIsAMDNode(value.as_ptr()).is_null() } {
            return Err(LLVMTypeError::InvalidPointerType("MDNode"));
        }
        Ok(Self { metadata, value })
    }

    fn as_metadata_ptr(&self) -> LLVMMetadataRef {
        self.metadata.as_ptr()
    }
}

impl LLVMTypeWrapper for MDNode {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        if unsafe { LLVMIsAMDNode(value.as_ptr()).is_null() } {
            return Err(LLVMTypeError::InvalidPointerType("MDNode"));
        }
        let metadata = unsafe { LLVMValueAsMetadata(value.as_ptr()) };
        let metadata =
            NonNull::new(metadata).expect("metadata of a non-null value should not be null");
        Ok(Self { metadata, value })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.value
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.value.as_ptr()
    }
}

impl LLVMTypeWrapperWithContext for MDNode {}

pub struct MetadataEntries {
    entries: NonNull<*mut LLVMOpaqueValueMetadataEntry>,
    count: usize,
}

impl MetadataEntries {
    pub fn new(v: NonNull<LLVMValue>) -> Option<Self> {
        if unsafe {
            LLVMIsAGlobalObject(v.as_ptr()).is_null() && LLVMIsAInstruction(v.as_ptr()).is_null()
        } {
            return None;
        }

        let mut count = 0;
        let entries = unsafe { LLVMGlobalCopyAllMetadata(v.as_ptr(), &mut count) };
        NonNull::new(entries).map(|entries| MetadataEntries { entries, count })
    }

    pub fn iter(&self) -> impl Iterator<Item = (LLVMMetadataRef, u32)> + '_ {
        (0..self.count).map(move |index| unsafe {
            (
                LLVMValueMetadataEntriesGetMetadata(self.entries.as_ptr(), index as u32),
                LLVMValueMetadataEntriesGetKind(self.entries.as_ptr(), index as u32),
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
