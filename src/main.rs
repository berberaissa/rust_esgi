#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::panic::PanicInfo;
use core::ptr::null_mut;

struct MyAllocator;

static mut HEAP: [u8; 1024] = [0; 1024];
static mut USED: bool = false;

unsafe impl GlobalAlloc for MyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if USED {
            return null_mut();
        }

        if layout.size() > 1024 {
            return null_mut();
        }

        USED = true;
        HEAP.as_mut_ptr()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        USED = false;
    }
}

#[global_allocator]
static ALLOCATOR: MyAllocator = MyAllocator;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}