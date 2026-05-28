use thiserror::Error;

pub(super) mod context;
pub(super) mod di;
pub(super) mod di_builder;
pub(super) mod ir;
pub(super) mod memory_buffer;
pub(super) mod module;
pub(super) mod target_machine;

#[derive(Debug, Error)]
pub(crate) enum LLVMTypeError {
    #[error("provided pointer for {0} is null")]
    NullPtr(&'static str),
    #[error("provided pointer is of an incorrect type")]
    IncorrectType,
}
