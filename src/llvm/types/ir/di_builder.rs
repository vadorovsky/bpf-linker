use std::{ffi::c_char, marker::PhantomData, ptr::NonNull};

use llvm_sys::{
    debuginfo::{
        LLVMDIBuilderCreateFunction, LLVMDIBuilderFinalizeSubprogram, LLVMDisposeDIBuilder,
    },
    LLVMContext, LLVMOpaqueDIBuilder,
};

use crate::llvm::{
    types::{
        ir::{DIFile, DIScope, DISubprogram, DISubroutineType},
        LLVMMetadataWrapper,
    },
    LLVMTypeWrapper,
};

pub struct DIBuilder {
    pub(crate) di_builder: NonNull<LLVMOpaqueDIBuilder>,
    // Unfortunately, LLVM C API doesn't support extracting the Context of a
    // DIBuiler. For now, it's the easiest to keep the LLVMContext pointer
    // around.
    pub(crate) context: NonNull<LLVMContext>,
}

impl Drop for DIBuilder {
    fn drop(&mut self) {
        unsafe { LLVMDisposeDIBuilder(self.di_builder.as_ptr()) }
    }
}

// Due to the necessity of carrying the `context` pointer, we can't simply
// implement `LLVMTypeWrapper<Target = LLVMOpaqueDIBuilder>` for `DIBuilder`.
// That's not a big issue, since we have no intention to convert raw
// `LLVMOpaqueDIBuilder` pointers to `DIBuilder` wrapeprs outside of the
// `Module::di_builder` method, where doing so manually is acceptable.
//
// However, we can still revisit implementing `LLVMTypeWrapper` once...

impl DIBuilder {
    pub fn create_function(
        &mut self,
        scope: &DIScope,
        name: &str,
        linkage_name: &str,
        file: &DIFile,
        line: u32,
        ty: &DISubroutineType,
        is_local_to_unit: bool,
        is_definition: bool,
        scope_line: u32,
        flags: i32,
        is_optimized: bool,
    ) -> DISubprogram {
        let function = unsafe {
            LLVMDIBuilderCreateFunction(
                self.di_builder.as_ptr(),
                scope.as_metadata_ptr(),
                name.as_ptr() as *const c_char,
                name.len(),
                linkage_name.as_ptr() as *const c_char,
                linkage_name.len(),
                file.as_ptr(),
                line,
                ty.as_metadata_ptr(),
                is_local_to_unit as i32,
                is_definition as i32,
                scope_line,
                flags,
                is_optimized as i32,
            )
        };
        let function = NonNull::new(function).expect("a new function should not be null");
        DISubprogram::from_metadata_ptr(function, self.context)
            .expect("a new function should be a valid pointer")
    }

    pub fn finalize_subprogram(&mut self, subprogram: &DISubprogram<'_>) {
        unsafe {
            LLVMDIBuilderFinalizeSubprogram(self.di_builder.as_ptr(), subprogram.as_metadata_ptr());
        }
    }
}
