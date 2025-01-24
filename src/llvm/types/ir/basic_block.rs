use std::ptr::NonNull;

use llvm_sys::LLVMBasicBlock;

use crate::llvm::types::{LLVMTypeError, LLVMTypeWrapper};

/// LLVM Basic Block Representation
///
/// This represents a single basic block in LLVM. A basic block is simply a
/// container of instructions that execute sequentially. Basic blocks are Values
/// because they are referenced by instructions such as branches and switch
/// tables. The type of a BasicBlock is "Type::LabelTy" because the basic block
/// represents a label to which a branch can jump.
///
/// A well formed basic block is formed of a list of non-terminating
/// instructions followed by a single terminator instruction. Terminator
/// instructions may not occur in the middle of basic blocks, and must terminate
/// the blocks. The BasicBlock class allows malformed basic blocks to occur
/// because it may be useful in the intermediate stage of constructing or
/// modifying a program. However, the verifier will ensure that basic blocks are
/// "well formed".
pub struct BasicBlock {
    value: NonNull<LLVMBasicBlock>,
}

impl LLVMTypeWrapper for BasicBlock {
    type Target = LLVMBasicBlock;

    fn from_ptr(value: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        Ok(Self { value })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.value
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.value.as_ptr()
    }
}
