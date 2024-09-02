pub mod di;
pub mod ir;
pub mod target;

pub trait LLVMTypeWrapper {
    type Target: ?Sized;

    unsafe fn from_ptr(ptr: Self::Target) -> Self;
    fn as_ptr(&self) -> Self::Target;
}
