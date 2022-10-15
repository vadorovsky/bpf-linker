use super::dw_tag::dw_tag_from_value_str;
use super::message::Message;
use super::symbol_name;
use crate::llvm::iter::*;
use gimli::DW_TAG_structure_type;
use llvm_sys::core::*;
use llvm_sys::debuginfo::*;
use llvm_sys::prelude::*;
use log::*;
use std::collections::HashSet;
use std::ffi::CStr;
use std::ffi::CString;

pub struct DIFix {
    context: LLVMContextRef,
    module: LLVMModuleRef,
    builder: LLVMDIBuilderRef,
    cache: Cache,
}

impl DIFix {
    pub unsafe fn new(context: LLVMContextRef, module: LLVMModuleRef) -> DIFix {
        DIFix {
            context,
            module,
            builder: LLVMCreateDIBuilder(module),
            cache: Cache::new(),
        }
    }

    unsafe fn mdnode(&mut self, value: LLVMValueRef) {
        let metadata = LLVMValueAsMetadata(value);
        let metadata_kind = LLVMGetMetadataKind(metadata);

        let empty = to_mdstring(self.context, "");

        match metadata_kind {
            LLVMMetadataKind::LLVMDICompositeTypeMetadataKind => {
                let msg = Message::from_ptr(LLVMPrintValueToString(value));
                let value_as_string = msg.to_str().unwrap_or("");
                let tag = dw_tag_from_value_str(value_as_string);

                #[allow(non_upper_case_globals)]
                match tag {
                    Some(DW_TAG_structure_type) => LLVMReplaceMDNodeOperandWith(value, 2, empty),
                    _ => (),
                }
            }
            LLVMMetadataKind::LLVMDIDerivedTypeMetadataKind => {
                LLVMReplaceMDNodeOperandWith(value, 2, empty);
            }
            _ => (),
        }
    }

    // navigate the tree of LLVMValueRefs (DFS-pre-order)
    unsafe fn discover(&mut self, value: LLVMValueRef, depth: usize) {
        let indent = indent(depth);

        if value.is_null() {
            trace!("{}skipping null node", indent);
            return;
        }

        // TODO: doing this on the pointer value is not good
        let key = if is_mdnode(value) {
            LLVMValueAsMetadata(value) as u64
        } else {
            value as u64
        };
        if self.cache.hit(&key) {
            trace!("{}skipping node", indent);
            return;
        }

        if is_mdnode(value) {
            let metadata = LLVMValueAsMetadata(value);
            let metadata_kind = LLVMGetMetadataKind(metadata);

            trace!(
                "{}mdnode kind:{:?} n_operands:{} value: {}",
                indent,
                metadata_kind,
                LLVMGetMDNodeNumOperands(value),
                Message::from_ptr(LLVMPrintValueToString(value))
                    .to_str()
                    .unwrap_or("")
            );

            self.mdnode(value)
        } else {
            trace!(
                "{}node value: {}",
                indent,
                Message::from_ptr(LLVMPrintValueToString(value))
                    .to_str()
                    .unwrap_or("")
            );
        }

        if can_get_all_metadata(value) {
            for (index, (kind, metadata)) in iter_medatada_copy(value).enumerate() {
                let metadata_value = LLVMMetadataAsValue(self.context, metadata);
                trace!("{}all_metadata entry: index:{}", indent, index);
                self.discover(metadata_value, depth + 1);

                if is_instruction(value) {
                    LLVMSetMetadata(value, kind, metadata_value);
                } else {
                    LLVMGlobalSetMetadata(value, kind, metadata);
                }
            }
        }

        if can_get_operands(value) {
            for (index, operand) in iter_operands(value).enumerate() {
                trace!(
                    "{}operand index:{} name:{} value:{}",
                    indent,
                    index,
                    symbol_name(value),
                    Message::from_ptr(LLVMPrintValueToString(value))
                        .to_str()
                        .unwrap_or("")
                );
                self.discover(operand, depth + 1)
            }
        }
    }

    pub unsafe fn run(&mut self) {
        for sym in self.module.named_metadata_iter() {
            let mut len: usize = 0;
            let name = CStr::from_ptr(LLVMGetNamedMetadataName(sym, &mut len))
                .to_str()
                .unwrap_or("");
            // just for debugging, we are not visiting those nodes for the moment
            trace!("named metadata name:{}", name);
        }

        let module = self.module;
        for (i, sym) in module.globals_iter().enumerate() {
            trace!("global index:{} name:{}", i, symbol_name(sym));
            self.discover(sym, 0);
        }

        for (i, sym) in module.global_aliases_iter().enumerate() {
            trace!("global aliases index:{} name:{}", i, symbol_name(sym));
            self.discover(sym, 0);
        }

        for function in module.functions_iter() {
            trace!("function > name:{}", symbol_name(function));
            self.discover(function, 0);

            let params_count = LLVMCountParams(function);
            for i in 0..params_count {
                let param = LLVMGetParam(function, i);
                trace!("function param name:{} index:{}", symbol_name(param), i);
                self.discover(param, 1);
            }

            for basic_block in function.basic_blocks_iter() {
                trace!("function block");
                for instruction in basic_block.instructions_iter() {
                    let n_operands = LLVMGetNumOperands(instruction);
                    trace!("function block instruction num_operands: {}", n_operands);
                    for index in 0..n_operands {
                        let operand = LLVMGetOperand(instruction, index as u32);
                        if is_instruction(operand) {
                            self.discover(operand, 2);
                        }
                    }

                    self.discover(instruction, 1);
                }
            }
        }

        LLVMDisposeDIBuilder(self.builder);
    }
}

// utils

unsafe fn to_mdstring(context: LLVMContextRef, chars: &str) -> LLVMMetadataRef {
    let cstr = CString::new(chars).unwrap();
    LLVMMDStringInContext2(context, cstr.as_ptr(), chars.len())
}

unsafe fn is_instruction(v: LLVMValueRef) -> bool {
    !LLVMIsAInstruction(v).is_null()
}

unsafe fn iter_operands(v: LLVMValueRef) -> impl Iterator<Item = LLVMValueRef> {
    (0..LLVMGetNumOperands(v)).map(move |i| LLVMGetOperand(v, i as u32))
}

unsafe fn iter_medatada_copy(v: LLVMValueRef) -> impl Iterator<Item = (u32, LLVMMetadataRef)> {
    let mut count = 0;
    let entries = LLVMGlobalCopyAllMetadata(v, &mut count);
    (0..count).map(move |index| {
        (
            LLVMValueMetadataEntriesGetKind(entries, index as u32),
            LLVMValueMetadataEntriesGetMetadata(entries, index as u32),
        )
    })
}

unsafe fn is_mdnode(v: LLVMValueRef) -> bool {
    !LLVMIsAMDNode(v).is_null()
}

unsafe fn is_user(v: LLVMValueRef) -> bool {
    !LLVMIsAUser(v).is_null()
}

unsafe fn is_globalobject(v: LLVMValueRef) -> bool {
    !LLVMIsAGlobalObject(v).is_null()
}

unsafe fn can_get_all_metadata(v: LLVMValueRef) -> bool {
    is_globalobject(v) || is_instruction(v)
}

unsafe fn can_get_operands(v: LLVMValueRef) -> bool {
    is_mdnode(v) || is_user(v)
}

fn indent(depth: usize) -> String {
    (0..depth).map(|_| "    ").collect::<Vec<&str>>().join("")
}

pub struct Cache {
    keys: HashSet<u64>,
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            keys: HashSet::new(),
        }
    }

    pub fn hit(&mut self, key: &u64) -> bool {
        if self.keys.contains(key) {
            return true;
        }
        self.keys.insert(key.clone());
        false
    }
}
