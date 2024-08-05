use std::ffi::CStr;

use llvm_sys::core::LLVMLookupIntrinsicID;

pub const LLVM_PRESERVE_ARRAY_ACCESS_INDEX_NAME: &CStr = c"llvm.preserve.struct.access.index";
pub const LLVM_PRESERVE_STRUCT_ACCESS_INDEX_NAME: &CStr = c"llvm.preserve.struct.access.index";

fn intrinsic_id(intrinsic_name: &CStr) -> u32 {
    unsafe { LLVMLookupIntrinsicID(intrinsic_name.as_ptr(), intrinsic_name.count_bytes()) }
}

pub fn llvm_preserve_array_access_index_id() -> u32 {
    intrinsic_id(LLVM_PRESERVE_ARRAY_ACCESS_INDEX_NAME)
}

pub fn llvm_preserve_struct_access_index_id() -> u32 {
    intrinsic_id(LLVM_PRESERVE_STRUCT_ACCESS_INDEX_NAME)
}
