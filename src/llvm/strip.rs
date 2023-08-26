use llvm_sys::core::*;
use llvm_sys::prelude::*;
use log::*;

use super::iter::*;
use super::{section, symbol_name};

const LLVM_MD_KIND_ID_DBG: u32 = 0;

pub fn strip_di(module: LLVMModuleRef) {
    for sym in module.globals_iter() {
        if section(sym).is_none() {
            trace!(
                "global {} does not have explicit link section, stripping debug info",
                symbol_name(sym)
            );
            strip(sym);
        }
    }

    for sym in module.global_aliases_iter() {
        if section(sym).is_none() {
            trace!(
                "global alias {} does not have explicit link section, stripping debug info",
                symbol_name(sym)
            );
            strip(sym);
        }
    }

    for function in module.functions_iter() {
        if section(function).is_none() {
            trace!(
                "function {}, does not have explicit link section, stripping debug info",
                symbol_name(function)
            );
            strip_all_children(function);
        }
    }
}

fn strip(value: LLVMValueRef) {
    unsafe { LLVMSetMetadata(value, LLVM_MD_KIND_ID_DBG, std::ptr::null_mut()) };
}

fn strip_all_children(value: LLVMValueRef) {
    for basic_block in value.basic_blocks_iter() {
        for instruction in basic_block.instructions_iter() {
            unsafe { LLVMSetMetadata(instruction, LLVM_MD_KIND_ID_DBG, std::ptr::null_mut()) };
        }
    }
    strip(value);
}
