use thiserror::Error;

pub mod di;
pub mod ir;

#[derive(Debug, Error)]
pub enum LLVMTypeError {
    #[error("cannot create a wrapper from a null pointer")]
    NullPointer,
    #[error("provided pointer doesn't represent a type of the wrapper")]
    InvalidPointerType,
}

pub trait LLVMTypeWrapper {
    type Target: ?Sized;

    /// Constructs a new LLVM type wrapper from the given raw pointer.
    ///
    /// # Safety
    ///
    /// Each implementation of `from_ptr` checks that the provided pointer
    /// corresponds to the type
    fn try_from_ptr(ptr: Self::Target) -> Result<Self, LLVMTypeError>
    where
        Self: Sized;
    /// Returns a raw pointer of the given LLVM type wrapper.
    fn as_ptr(&self) -> Self::Target;
}
