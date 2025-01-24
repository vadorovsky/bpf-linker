use std::ptr::NonNull;

use llvm_sys::{prelude::LLVMMetadataRef, LLVMContext, LLVMOpaqueMetadata, LLVMValue};
use thiserror::Error;

pub mod ir;
pub mod target;

#[derive(Debug, Error)]
pub enum LLVMTypeError {
    #[error("invalid pointer type, expected {0}")]
    InvalidPointerType(&'static str),
    #[error("null pointer")]
    NullPointer,
}

pub trait LLVMMetadataWrapper: LLVMTypeWrapper<Target = LLVMValue> {
    /// Constructs a new [`Self`] from the given `metadata` and `context`
    /// pointers.
    fn from_metadata_ptr(
        metadata: NonNull<LLVMOpaqueMetadata>,
        context: NonNull<LLVMContext>,
    ) -> Result<Self, LLVMTypeError>
    where
        Self: Sized;
    /// Returns a raw pointer to the LLVM metadata.
    fn as_metadata_ptr(&self) -> LLVMMetadataRef;
}

pub trait LLVMTypeWrapper {
    type Target: Sized;

    /// Constructs a new [`Self`] from the given pointer `ptr`.
    fn from_ptr(ptr: NonNull<Self::Target>) -> Result<Self, LLVMTypeError>
    where
        Self: Sized;
    /// Returns a [`NonNull`] wrapping a raw pointer to the LLVM type.
    fn as_non_null(&self) -> NonNull<Self::Target>;
    /// Returns a raw pointer to the LLVM type.
    fn as_ptr(&self) -> *mut Self::Target;
}
