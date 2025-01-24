use std::{
    borrow::Cow,
    ffi::{c_uchar, CStr, CString},
    ptr::NonNull,
    slice,
};

use llvm_sys::{
    core::{
        LLVMDisposeModule, LLVMGetModuleContext, LLVMGetModuleInlineAsm, LLVMGetTarget,
        LLVMSetModuleInlineAsm2,
    },
    debuginfo::LLVMCreateDIBuilder,
    LLVMContext, LLVMModule,
};

use crate::llvm::{types::ir::DIBuilder, LLVMTypeError, LLVMTypeWrapper};

/// A top level container of all other LLVM IR objects.
///
/// Each module directly contains a list of globals variables, a list of
/// functions, a list of libraries (or other modules) this module depends on, a
/// symbol table, and various data about the target's characteristics.
pub struct Module {
    pub(crate) module: NonNull<LLVMModule>,
    // Many operations done on a module require a context. Requiring callers to
    // always pass a reference to context would lead to an API which is
    // difficult to maintain and use. Requiring mutable references to `Module`
    // and its parent `Context` at once would .
    //
    // `Module` is always created in a context. If a `Module` was created using
    // the safe API (`Context::create_module`), the `Context` manages the
    // lifecycle of the context and keeps the owned `Module` internally.
    // `Module` doesn't have to care about disposing the context pointer, because
    // dropping `Context` will achieve that.
    pub(crate) context: NonNull<LLVMContext>,
    // Keep an owned DI builder and return only its references to the callers
    // to make sure they are dropped before the context.
    pub(crate) di_builder: Option<DIBuilder>,
}

impl Drop for Module {
    fn drop(&mut self) {
        println!("dropping module");
        unsafe { LLVMDisposeModule(self.module.as_ptr()) }
    }
}

impl LLVMTypeWrapper for Module {
    type Target = LLVMModule;

    fn from_ptr(module: NonNull<Self::Target>) -> Result<Self, LLVMTypeError>
    where
        Self: Sized,
    {
        let context = unsafe { LLVMGetModuleContext(module.as_ptr()) };
        let context = NonNull::new(context).expect("context of a module should not be null");
        Ok(Self {
            module,
            context,
            di_builder: None,
        })
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        self.module
    }

    fn as_ptr(&self) -> *mut Self::Target {
        self.module.as_ptr()
    }
}

impl Module {
    pub fn di_builder(&mut self) -> &mut DIBuilder {
        if self.di_builder.is_none() {
            let di_builder = unsafe { LLVMCreateDIBuilder(self.as_ptr()) };
            let di_builder = NonNull::new(di_builder).expect("a new DI builder should not be null");
            self.di_builder = Some(DIBuilder {
                di_builder,
                context: self.context,
            });
        }
        self.di_builder.as_mut().unwrap()
    }

    pub fn inline_asm(&self) -> Cow<'_, str> {
        let mut len = 0;
        let ptr = unsafe { LLVMGetModuleInlineAsm(self.module.as_ptr(), &mut len) };
        let asm = unsafe { slice::from_raw_parts(ptr as *const c_uchar, len) };
        String::from_utf8_lossy(asm)
    }

    pub fn set_inline_asm(&mut self, asm: &str) {
        let len = asm.len();
        let asm = CString::new(asm).unwrap();
        unsafe {
            LLVMSetModuleInlineAsm2(self.module.as_ptr(), asm.as_ptr(), len);
        }
    }

    pub fn target_triple(&self) -> Cow<'_, str> {
        let triple = unsafe { LLVMGetTarget(self.as_ptr()) };
        unsafe { CStr::from_ptr(triple).to_string_lossy() }
    }
}
