// assembly-output: bpf-linker
// compile-flags: --crate-type cdylib -C link-arg=--emit=llvm-ir -C link-arg=--btf -C debuginfo=2

#![no_std]

use core::{ffi::c_void, panic::PanicInfo};

unsafe extern "C" {
    fn relocatable_preserve_access_index(ptr: *const c_void) -> *const c_void;
}

#[inline(always)]
unsafe fn preserve_access_index<T>(ptr: *const T) -> *const T {
    unsafe { relocatable_preserve_access_index(ptr.cast()) }.cast()
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {}
}

#[repr(C)]
pub struct Foo {
    a: u32,
    b: u32,
}

// CHECK: @"llvm.Foo:0:4$0:1" = external global i64, !llvm.preserve.access.index ![[FOO:[0-9]+]] #[[AMA:[0-9]+]]

// CHECK-LABEL: define i32 @get_b(
#[no_mangle]
#[link_section = "uprobe/get_b"]
pub unsafe extern "C" fn get_b(x: *mut c_void) -> u32 {
    let x: *const Foo = x.cast();
    // CHECK: %[[OFF:[0-9]+]] = load i64, ptr @"llvm.Foo:0:4$0:1", align 8
    // CHECK-NEXT: %[[FIELD_PTR:[0-9]+]] = getelementptr i8, ptr %{{.*}}, i64 %[[OFF]]
    // CHECK-NEXT: %[[PASSTHROUGH:[0-9]+]] = tail call ptr @llvm.bpf.passthrough{{.*}}(i32 {{[0-9]+}}, ptr %[[FIELD_PTR]])
    // CHECK: %{{.*}} = load i32, ptr %[[PASSTHROUGH]], align 4
    *preserve_access_index(core::ptr::addr_of!((*x).b))
}

// CHECK: attributes #[[AMA]] = { "btf_ama" }
// CHECK: ![[FOO]] = !DICompositeType(tag: DW_TAG_structure_type, name: "Foo"
// CHECK-SAME: elements: ![[FOO_ELTS:[0-9]+]]
// CHECK: ![[FOO_ELTS]] = !{![[A:[0-9]+]], ![[B:[0-9]+]]}
// CHECK: ![[A]] = !DIDerivedType(tag: DW_TAG_member, name: "a"
// CHECK: ![[B]] = !DIDerivedType(tag: DW_TAG_member, name: "b"
