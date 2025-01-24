use std::{
    ffi::{CString, NulError},
    ptr::{self, NonNull},
};

use llvm_sys::target_machine::{
    LLVMCodeGenOptLevel, LLVMCodeModel, LLVMCreateTargetMachine, LLVMGetTargetFromTriple,
    LLVMOpaqueTargetMachine, LLVMRelocMode, LLVMTarget,
};

use crate::llvm::Message;

use super::LLVMTypeWrapper;

/// Target specific information.
pub struct Target {
    target: NonNull<LLVMTarget>,
}

impl LLVMTypeWrapper for Target {
    type Target = LLVMTarget;

    fn from_ptr(target: NonNull<Self::Target>) -> Result<Self, super::LLVMTypeError>
    where
        Self: Sized,
    {
        Ok(Self { target })
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.target.as_ptr()
    }
}

impl Target {
    pub fn from_triple(triple: &str) -> Result<Self, String> {
        let triple = CString::new(triple).unwrap();
        let mut target = ptr::null_mut();
        let (ret, message) = Message::with(|message| unsafe {
            LLVMGetTargetFromTriple(triple.as_ptr(), &mut target, message)
        });
        if ret == 0 {
            let target = NonNull::new(target).expect("new target should not be null");
            let target = Target::from_ptr(target).expect("new target should be a valid pointer");
            Ok(target)
        } else {
            Err(message.as_c_str().unwrap().to_str().unwrap().to_string())
        }
    }

    pub fn create_target_machine(
        &self,
        triple: &str,
        cpu: &str,
        features: &str,
        level: LLVMCodeGenOptLevel,
        reloc: LLVMRelocMode,
        code_model: LLVMCodeModel,
    ) -> Result<TargetMachine, NulError> {
        let triple = CString::new(triple)?;
        let cpu = CString::new(cpu)?;
        let features = CString::new(features)?;
        let target_machine = unsafe {
            LLVMCreateTargetMachine(
                self.as_ptr(),
                triple.as_ptr(),
                cpu.as_ptr(),
                features.as_ptr(),
                level,
                reloc,
                code_model,
            )
        };
        let target_machine =
            NonNull::new(target_machine).expect("a new target machine should not be null");
        let target_machine = TargetMachine::from_ptr(target_machine)
            .expect("a new target machine should be a valid pointer");
        Ok(target_machine)
    }
}

/// Complete machine description for the target machine. All target-specific
/// information should be accessible through this interface.
pub struct TargetMachine {
    target_machine: NonNull<LLVMOpaqueTargetMachine>,
}

impl LLVMTypeWrapper for TargetMachine {
    type Target = LLVMOpaqueTargetMachine;

    fn from_ptr(target_machine: NonNull<Self::Target>) -> Result<Self, super::LLVMTypeError>
    where
        Self: Sized,
    {
        Ok(Self { target_machine })
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.target_machine.as_ptr()
    }
}
