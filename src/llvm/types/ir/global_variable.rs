use std::ptr::NonNull;

use llvm_sys::{core::LLVMIsAGlobalVariable, LLVMValue};

use crate::llvm::types::{
    ir::{GlobalValue, NamedValue},
    LLVMTypeError, LLVMTypeWrapper,
};

/// Represents a global variable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalVariable {
    value: NonNull<LLVMValue>,
}

impl LLVMTypeWrapper for GlobalVariable {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        if unsafe { LLVMIsAGlobalVariable(value.as_ptr()).is_null() } {
            return Err(LLVMTypeError::InvalidPointerType("GlobalVariable"));
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

impl GlobalValue for GlobalVariable {}
impl NamedValue for GlobalVariable {}
