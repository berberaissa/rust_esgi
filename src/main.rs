#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::{Cell, UnsafeCell};
use core::panic::PanicInfo;
use core::ptr::null_mut;


struct MyAllocator {
    heap: UnsafeCell<[u8; 1024]>,
    used: Cell<bool>,
}

// Required because this allocator is stored in a global static.
// We are promising we know what we are doing.
unsafe impl Sync for MyAllocator {}

unsafe impl GlobalAlloc for MyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.used.get() {
            return null_mut();
        }

        if layout.size() > 1024 {
            return null_mut();
        }

        self.used.set(true);

        // Get a raw pointer to the start of the heap
        (*self.heap.get()).as_mut_ptr()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        self.used.set(false);
    }
}

#[global_allocator]
static ALLOCATOR: MyAllocator = MyAllocator {
    heap: UnsafeCell::new([0; 1024]),
    used: Cell::new(false),
};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}