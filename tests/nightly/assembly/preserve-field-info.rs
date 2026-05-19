// assembly-output: bpf-linker
// compile-flags: --crate-type cdylib -C link-arg=--emit=llvm-ir -C link-arg=--btf -C debuginfo=2

#![no_std]

use core::{ffi::c_void, panic::PanicInfo};

const FIELD_BYTE_OFFSET: u32 = 0;
const FIELD_BYTE_SIZE: u32 = 1;
const FIELD_EXISTS: u32 = 2;

#[inline(always)]
fn relocatable_field_info<T>(ptr: *const T, kind: u32) -> u32 {
    unsafe extern "C" {
        fn relocatable_field_info(ptr: *const c_void, kind: u32) -> u32;
    }
    unsafe { relocatable_field_info(ptr.cast(), kind) }
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

// CHECK: @"llvm.Foo:2:1$0:0" = external global i32, !llvm.preserve.access.index ![[FOO:[0-9]+]] #[[AMA:[0-9]+]]
// CHECK: @"llvm.Foo:0:4$0:1" = external global i32, !llvm.preserve.access.index ![[FOO]] #[[AMA]]
// CHECK: @"llvm.Foo:1:4$0:1" = external global i32, !llvm.preserve.access.index ![[FOO]] #[[AMA]]

// CHECK-LABEL: define i32 @field_exists_a(
#[no_mangle]
#[link_section = "uprobe/field_exists_a"]
pub unsafe extern "C" fn field_exists_a(x: *const Foo) -> u32 {
    // CHECK: %{{.*}} = load i32, ptr @"llvm.Foo:2:1$0:0", align 4
    // CHECK-NEXT: %{{.*}} = tail call i32 @llvm.bpf.passthrough{{.*}}(i32 {{[0-9]+}}, i32 %{{.*}})
    relocatable_field_info(core::ptr::addr_of!((*x).a), FIELD_EXISTS)
}

// CHECK-LABEL: define i32 @field_offset_b(
#[no_mangle]
#[link_section = "uprobe/field_offset_b"]
pub unsafe extern "C" fn field_offset_b(x: *const Foo) -> u32 {
    // CHECK: %{{.*}} = load i32, ptr @"llvm.Foo:0:4$0:1", align 4
    // CHECK-NEXT: %{{.*}} = tail call i32 @llvm.bpf.passthrough{{.*}}(i32 {{[0-9]+}}, i32 %{{.*}})
    relocatable_field_info(core::ptr::addr_of!((*x).b), FIELD_BYTE_OFFSET)
}

// CHECK-LABEL: define i32 @field_size_b(
#[no_mangle]
#[link_section = "uprobe/field_size_b"]
pub unsafe extern "C" fn field_size_b(x: *const Foo) -> u32 {
    // CHECK: %{{.*}} = load i32, ptr @"llvm.Foo:1:4$0:1", align 4
    // CHECK-NEXT: %{{.*}} = tail call i32 @llvm.bpf.passthrough{{.*}}(i32 {{[0-9]+}}, i32 %{{.*}})
    relocatable_field_info(core::ptr::addr_of!((*x).b), FIELD_BYTE_SIZE)
}

// CHECK: attributes #[[AMA]] = { "btf_ama" }
// CHECK: ![[FOO]] = !DICompositeType(tag: DW_TAG_structure_type, name: "Foo"
