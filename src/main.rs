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

fn exit(code: i32) -> ! {
    unsafe {
        core::arch::asm!(
            "mov eax, 60",
            "syscall",
            in("edi") code,
            options(noreturn)
        );
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        let layout = Layout::from_size_align(64, 8).unwrap();

        let mut ptrs = [core::ptr::null_mut(); 8];

        let mut i = 0;
        while i < 8 {
            let p = ALLOCATOR.alloc(layout);
            if p.is_null() {
                exit(1);
            }
            ptrs[i] = p;
            i += 1;
        }

        let extra = ALLOCATOR.alloc(layout);
        if !extra.is_null() {
            exit(2);
        }

        ALLOCATOR.dealloc(ptrs[0], layout);

        let p_new = ALLOCATOR.alloc(layout);
        if p_new.is_null() {
            exit(3);
        }
    }

    exit(0);
}
