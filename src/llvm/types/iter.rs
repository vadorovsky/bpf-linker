use std::marker::PhantomData;

use llvm_sys::{
    core::{
        LLVMGetFirstBasicBlock, LLVMGetFirstDbgRecord, LLVMGetFirstFunction, LLVMGetFirstGlobal,
        LLVMGetFirstGlobalAlias, LLVMGetFirstInstruction, LLVMGetLastBasicBlock,
        LLVMGetLastDbgRecord, LLVMGetLastFunction, LLVMGetLastGlobal, LLVMGetLastGlobalAlias,
        LLVMGetLastInstruction, LLVMGetNextBasicBlock, LLVMGetNextDbgRecord, LLVMGetNextFunction,
        LLVMGetNextGlobal, LLVMGetNextGlobalAlias, LLVMGetNextInstruction,
    },
    prelude::{LLVMBasicBlockRef, LLVMDbgRecordRef, LLVMModuleRef, LLVMValueRef},
};

use crate::llvm::types::ir::{
    BasicBlock, DbgRecord, Function, Instruction, Module, Value, ValueRef,
};

macro_rules! llvm_iterator {
    (
        $trait_name:ident,
        $iterator_name:ident,
        $iterable:ident,
        $method_name:ident,
        $item_ty:ty,
        $first:expr,
        $last:expr,
        $next:expr,
        $ref_method:ident $(,)?
    ) => {
        pub trait $trait_name {
            fn $method_name(&self) -> $iterator_name;
        }

        pub struct $iterator_name<'a> {
            lifetime: PhantomData<&'a $iterable<'a>>,
            next: $item_ty,
            last: $item_ty,
        }

        impl<'ctx> $trait_name for $iterable<'ctx> {
            fn $method_name(&self) -> $iterator_name {
                let first = unsafe { $first(self.$ref_method()) };
                let last = unsafe { $last(self.$ref_method()) };
                assert_eq!(first.is_null(), last.is_null());
                $iterator_name {
                    lifetime: PhantomData,
                    next: first,
                    last,
                }
            }
        }

        impl<'a> Iterator for $iterator_name<'a> {
            type Item = $item_ty;

            fn next(&mut self) -> Option<Self::Item> {
                let Self {
                    lifetime: _,
                    next,
                    last,
                } = self;
                if next.is_null() {
                    return None;
                }
                let last = *next == *last;
                let item = *next;
                *next = unsafe { $next(*next) };
                assert_eq!(next.is_null(), last);
                Some(item)
            }
        }
    };
}

llvm_iterator! {
    IterModuleGlobals,
    GlobalsIter,
    Module,
    globals_iter,
    LLVMValueRef,
    LLVMGetFirstGlobal,
    LLVMGetLastGlobal,
    LLVMGetNextGlobal,
    module_ref,
}

llvm_iterator! {
    IterModuleGlobalAliases,
    GlobalAliasesIter,
    Module,
    global_aliases_iter,
    LLVMValueRef,
    LLVMGetFirstGlobalAlias,
    LLVMGetLastGlobalAlias,
    LLVMGetNextGlobalAlias,
    module_ref,
}

llvm_iterator! {
    IterModuleFunctions,
    FunctionsIter,
    Module,
    functions_iter,
    LLVMValueRef,
    LLVMGetFirstFunction,
    LLVMGetLastFunction,
    LLVMGetNextFunction,
    module_ref,
}

llvm_iterator!(
    IterBasicBlocks,
    BasicBlockIter,
    Function,
    basic_blocks_iter,
    LLVMBasicBlockRef,
    LLVMGetFirstBasicBlock,
    LLVMGetLastBasicBlock,
    LLVMGetNextBasicBlock,
    value_ref,
);

llvm_iterator!(
    IterInstructions,
    InstructionsIter,
    BasicBlock,
    instructions_iter,
    LLVMValueRef,
    LLVMGetFirstInstruction,
    LLVMGetLastInstruction,
    LLVMGetNextInstruction,
    basic_block_ref,
);

llvm_iterator!(
    IterDbgRecords,
    DbgRecordsIter,
    // LLVMValueRef,
    Instruction,
    dbg_records_iter,
    LLVMDbgRecordRef,
    LLVMGetFirstDbgRecord,
    LLVMGetLastDbgRecord,
    LLVMGetNextDbgRecord,
    value_ref,
);
