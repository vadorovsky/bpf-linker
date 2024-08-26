#![deny(clippy::all)]
#![deny(unused_results)]

mod di_sanitizer;
mod linker;
mod llvm;

pub use linker::*;
