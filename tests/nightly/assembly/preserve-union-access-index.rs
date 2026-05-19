// assembly-output: bpf-linker
// compile-flags: --crate-type cdylib -C link-arg=--emit=llvm-ir -C link-arg=--btf -C debuginfo=2

#![no_std]

use core::panic::PanicInfo;

unsafe extern "C" {
    fn relocatable_preserve_access_index(ptr: *const u8) -> *const u8;
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
pub union Foo {
    a: u32,
    b: u64,
}

// CHECK: @"llvm.Foo{{.*}}" = external global i64, !llvm.preserve.access.index ![[FOO:[0-9]+]] #[[AMA:[0-9]+]]

// CHECK-LABEL: define i64 @get_b(
#[no_mangle]
#[link_section = "uprobe/get_b"]
pub unsafe extern "C" fn get_b(x: *const Foo) -> u64 {
    // CHECK: %[[OFF:[0-9]+]] = load i64, ptr @"llvm.Foo{{.*}}", align 8
    // CHECK-NEXT: %[[FIELD_PTR:[0-9]+]] = getelementptr i8, ptr %{{.*}}, i64 %[[OFF]]
    // CHECK-NEXT: %[[PASSTHROUGH:[0-9]+]] = tail call ptr @llvm.bpf.passthrough{{.*}}(i32 {{[0-9]+}}, ptr %[[FIELD_PTR]])
    // CHECK: %{{.*}} = load i64, ptr %[[PASSTHROUGH]], align 8
    *preserve_access_index(core::ptr::addr_of!((*x).b))
}

// CHECK: attributes #[[AMA]] = { "btf_ama" }
// CHECK: ![[FOO]] = !DICompositeType(tag: DW_TAG_union_type, name: "Foo"
