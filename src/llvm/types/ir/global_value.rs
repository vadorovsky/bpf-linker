use llvm_sys::{
    core::{LLVMSetLinkage, LLVMSetVisibility},
    LLVMLinkage, LLVMValue, LLVMVisibility,
};

use crate::llvm::types::LLVMTypeWrapper;

pub trait GlobalValue: LLVMTypeWrapper<Target = LLVMValue> {
    fn set_linkage(&mut self, linkage: LLVMLinkage) {
        unsafe {
            LLVMSetLinkage(self.as_ptr(), linkage);
        }
    }

    fn set_visibility(&mut self, visibility: LLVMVisibility) {
        unsafe {
            LLVMSetVisibility(self.as_ptr(), visibility);
        }
    }
}
