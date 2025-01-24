use std::ffi::{CString, NulError};

use llvm_sys::{
    core::{LLVMMDStringInContext2, LLVMReplaceMDNodeOperandWith},
    prelude::{LLVMContextRef, LLVMValueRef},
};

mod argument;
mod basic_block;
mod context;
mod debug_info_metadata;
mod di_builder;
mod function;
mod global_alias;
mod global_value;
mod global_variable;
mod instruction;
mod metadata;
mod module;
mod value;

pub use argument::Argument;
pub use basic_block::BasicBlock;
pub use context::{Context, LLVMContextWrapper, LLVMTypeWrapperWithContext};
pub use debug_info_metadata::{
    DICompositeType, DIDerivedType, DIFile, DIScope, DISubprogram, DISubroutineType, DIType,
};
pub use di_builder::DIBuilder;
pub use function::Function;
pub use global_alias::GlobalAlias;
pub use global_value::GlobalValue;
pub use global_variable::GlobalVariable;
pub use instruction::Instruction;
pub use metadata::{MDNode, Metadata};
pub use module::Module;
pub use value::{NamedValue, Value};

pub(crate) fn replace_name(
    value_ref: LLVMValueRef,
    context: LLVMContextRef,
    name_operand_index: u32,
    name: &str,
) -> Result<(), NulError> {
    let cstr = CString::new(name)?;
    let name = unsafe { LLVMMDStringInContext2(context, cstr.as_ptr(), name.len()) };
    unsafe { LLVMReplaceMDNodeOperandWith(value_ref, name_operand_index, name) };
    Ok(())
}
