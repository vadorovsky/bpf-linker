use std::{
    collections::HashMap,
    ffi::{CString, NulError},
    ptr::{self, NonNull},
};

use llvm_sys::{
    core::{
        LLVMContextCreate, LLVMContextDispose, LLVMGetTypeContext, LLVMMDNodeInContext2,
        LLVMModuleCreateWithNameInContext, LLVMTypeOf, LLVMValueAsMetadata,
    },
    prelude::LLVMMetadataRef,
    LLVMContext, LLVMValue,
};

use crate::llvm::{
    types::{
        ir::{DIType, MDNode, Module},
        LLVMMetadataWrapper,
    },
    LLVMTypeError, LLVMTypeWrapper,
};

/// Owner and manager of the LLVM-related data.
///
/// It (opaquely) owns and manages the core "global" data of LLVM's core
/// infrastructure, including the type and constant uniquing tables. It does
/// not provide any locking guarantees, therefore it's not thread-safe and
/// a signle context should be used in a single thread.
pub struct Context {
    pub(crate) context: NonNull<LLVMContext>,
    // Keep owned modules in a map and return only references to the callers
    // to make sure they are dropped before the context.
    pub(crate) modules: HashMap<String, Module>,
}

impl Default for Context {
    fn default() -> Self {
        let context = unsafe { LLVMContextCreate() };
        let context = NonNull::new(context).expect("new context should not be null");
        Self {
            context,
            modules: HashMap::new(),
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        println!("dropping context");
        unsafe { LLVMContextDispose(self.context.as_ptr()) }
    }
}

impl LLVMTypeWrapper for Context {
    type Target = LLVMContext;

    fn from_ptr(context: NonNull<Self::Target>) -> Result<Self, LLVMTypeError>
    where
        Self: Sized,
    {
        Ok(Self {
            context,
            modules: HashMap::new(),
        })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.context
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.context.as_ptr()
    }
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_module(&mut self, name: &str) -> Result<(), NulError> {
        let c_name = CString::new(name)?;
        let module = unsafe { LLVMModuleCreateWithNameInContext(c_name.as_ptr(), self.as_ptr()) };
        let module = NonNull::new(module).expect("a new module should not be null");
        let module = Module {
            module,
            context: self.context,
            di_builder: None,
        };
        self.modules.insert(name.to_owned(), module);
        Ok(())
    }

    pub fn module(&self, name: &str) -> Option<&Module> {
        self.modules.get(name)
    }

    pub fn module_mut(&mut self, name: &str) -> Option<&mut Module> {
        self.modules.get_mut(name)
    }
}

/// A non-owned reference to LLVM context. See [`Context`].
pub struct ContextRef {
    context: NonNull<LLVMContext>,
}

impl LLVMTypeWrapper for ContextRef {
    type Target = LLVMContext;

    fn from_ptr(context: NonNull<Self::Target>) -> Result<Self, LLVMTypeError>
    where
        Self: Sized,
    {
        Ok(Self { context })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.context
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.context.as_ptr()
    }
}

pub trait LLVMContextWrapper: LLVMTypeWrapper<Target = LLVMContext> {
    fn create_mdnode(&mut self, elements: &[DIType]) -> MDNode {
        let metadata = unsafe {
            if elements.is_empty() {
                LLVMMDNodeInContext2(self.as_ptr(), ptr::null_mut(), 0)
            } else {
                let mut elements: Vec<LLVMMetadataRef> = elements
                    .iter()
                    .map(|di_type| LLVMValueAsMetadata(di_type.as_ptr()))
                    .collect();
                LLVMMDNodeInContext2(
                    self.as_ptr(),
                    elements.as_mut_slice().as_mut_ptr(),
                    elements.len(),
                )
            }
        };
        let metadata = NonNull::new(metadata).expect("new MDNode should not be null");
        MDNode::from_metadata_ptr(metadata, self.as_non_null()).expect("expected a valid MDNode")
    }
}

impl LLVMContextWrapper for Context {}
impl LLVMContextWrapper for ContextRef {}

pub trait LLVMTypeWrapperWithContext: LLVMTypeWrapper<Target = LLVMValue> {
    fn context(&self) -> ContextRef {
        let context = unsafe { LLVMGetTypeContext(LLVMTypeOf(self.as_ptr())) };
        let context = NonNull::new(context).expect("context should not be null");
        ContextRef { context }
    }
}
