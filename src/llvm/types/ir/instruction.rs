use std::ptr::NonNull;

use llvm_sys::LLVMValue;

use crate::llvm::types::{LLVMTypeError, LLVMTypeWrapper};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Instruction {
    value: NonNull<LLVMValue>,
}

impl LLVMTypeWrapper for Instruction {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError>
    where
        Self: Sized,
    {
        Ok(Self { value })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.value
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.value.as_ptr()
    }
}
