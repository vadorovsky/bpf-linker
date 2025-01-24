use std::ptr::NonNull;

use llvm_sys::{core::LLVMIsAGlobalAlias, LLVMValue};

use crate::llvm::types::{
    ir::{GlobalValue, NamedValue},
    LLVMTypeError, LLVMTypeWrapper,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalAlias {
    value: NonNull<LLVMValue>,
}

impl LLVMTypeWrapper for GlobalAlias {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        if unsafe { LLVMIsAGlobalAlias(value.as_ptr()).is_null() } {
            return Err(LLVMTypeError::InvalidPointerType("GlobalAlias"));
        }
        Ok(Self { value })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.value
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.value.as_ptr()
    }
}

impl GlobalValue for GlobalAlias {}
impl NamedValue for GlobalAlias {}
