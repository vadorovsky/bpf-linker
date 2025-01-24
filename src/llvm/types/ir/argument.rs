use std::ptr::NonNull;

use llvm_sys::{core::LLVMIsAArgument, LLVMValue};

use crate::llvm::types::{LLVMTypeError, LLVMTypeWrapper};

/// Formal argument to a [`Function`].
///
/// This class represents an incoming formal argument to a Function. A formal
/// argument, since it is ``formal'', does not contain an actual value but
/// instead represents the type, argument number, and attributes of an argument
/// for a specific function. When used in the body of said function, the
/// argument of course represents the value of the actual argument that the
/// function was called with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Argument {
    value: NonNull<LLVMValue>,
}

impl LLVMTypeWrapper for Argument {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError>
    where
        Self: Sized,
    {
        if unsafe { LLVMIsAArgument(value.as_ptr()).is_null() } {
            return Err(LLVMTypeError::InvalidPointerType("Argument"));
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
