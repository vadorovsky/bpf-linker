use std::ptr::NonNull;

use llvm_sys::{
    core::{
        LLVMCountParams, LLVMGetParam, LLVMGetTypeContext, LLVMIsAFunction, LLVMMetadataAsValue,
        LLVMTypeOf, LLVMValueAsMetadata,
    },
    debuginfo::{LLVMGetSubprogram, LLVMSetSubprogram},
    LLVMValue,
};

use crate::llvm::types::{
    ir::{Argument, DISubprogram, GlobalValue, NamedValue},
    LLVMTypeError, LLVMTypeWrapper,
};

/// Represents a function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    value: NonNull<LLVMValue>,
}

impl LLVMTypeWrapper for Function {
    type Target = LLVMValue;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        if unsafe { LLVMIsAFunction(value.as_ptr()).is_null() } {
            return Err(LLVMTypeError::InvalidPointerType("Function"));
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

impl GlobalValue for Function {}
impl NamedValue for Function {}

impl Function {
    pub(crate) fn params(&self) -> impl Iterator<Item = Argument> {
        let params_count = unsafe { LLVMCountParams(self.value.as_ptr()) };
        let value = self.value.as_ptr();
        (0..params_count).map(move |i| {
            let ptr = unsafe { LLVMGetParam(value, i) };
            Argument::from_ptr(NonNull::new(ptr).expect("an argument should not be null")).unwrap()
        })
    }

    pub(crate) fn subprogram(&self) -> Option<DISubprogram> {
        let subprogram = unsafe { LLVMGetSubprogram(self.value.as_ptr()) };
        let subprogram = NonNull::new(subprogram)?;
        let context = unsafe { LLVMGetTypeContext(LLVMTypeOf(self.as_ptr())) };
        let value = unsafe { LLVMMetadataAsValue(context, subprogram.as_ptr()) };
        let value = NonNull::new(value)?;
        Some(DISubprogram::from_ptr(value).unwrap())
    }

    pub(crate) fn set_subprogram(&mut self, subprogram: &DISubprogram) {
        unsafe {
            LLVMSetSubprogram(
                self.value.as_ptr(),
                LLVMValueAsMetadata(subprogram.as_ptr()),
            )
        };
    }
}
