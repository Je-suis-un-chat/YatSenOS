#![no_std]
#![no_main]

#[macro_use]
extern crate log;

use core::arch::asm;

use boot::{config, fs};
use ysos_kernel as ysos;

boot::entry_point!(kernel_main);

pub fn kernel_main(boot_info: &'static boot::BootInfo) -> ! {
    ysos::init(boot_info);

        info!("Hello World from YatSenOS v2!");
        // 找个合适的地方（比如初始化全部完成之后）
       panic!("Don't panic! This is just a test for YatSenOS.");
    
}


struct DummyAllocator;

unsafe impl core::alloc::GlobalAlloc for DummyAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

#[global_allocator]
static ALLOCATOR: DummyAllocator = DummyAllocator;


