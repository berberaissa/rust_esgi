#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
<<<<<<< HEAD
use core::cell::UnsafeCell;
use core::panic::PanicInfo;
use core::ptr::null_mut;

const HEAP_SIZE: usize = 1024;
const SLOT_SIZE: usize = 128;
const NUM_SLOTS: usize = HEAP_SIZE / SLOT_SIZE;

struct MyAllocator {
    heap: UnsafeCell<[u8; HEAP_SIZE]>,
    used_slots: UnsafeCell<[bool; NUM_SLOTS]>,
}

=======
use core::cell::{Cell, UnsafeCell};
use core::panic::PanicInfo;
use core::ptr::null_mut;

struct MyAllocator {
    heap: UnsafeCell<[u8; 1024]>,
    used: Cell<bool>,
}

// Required because this allocator is stored in a global static.
// We are promising we know what we are doing.
>>>>>>> 01a949feee4aae2cf6186c16b74725448c062c13
unsafe impl Sync for MyAllocator {}

unsafe impl GlobalAlloc for MyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
<<<<<<< HEAD
        if layout.size() > SLOT_SIZE {
=======
        if self.used.get() {
>>>>>>> 01a949feee4aae2cf6186c16b74725448c062c13
            return null_mut();
        }

        let used = &mut *self.used_slots.get();

        let mut i = 0;
        while i < NUM_SLOTS {
            if !used[i] {
                used[i] = true;

                let heap_ptr = (*self.heap.get()).as_mut_ptr();
                return heap_ptr.add(i * SLOT_SIZE);
            }
            i += 1;
        }

<<<<<<< HEAD
        null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let heap_start = (*self.heap.get()).as_mut_ptr() as usize;
        let ptr_addr = ptr as usize;

        if ptr_addr < heap_start || ptr_addr >= heap_start + HEAP_SIZE {
            return;
        }

        let offset = ptr_addr - heap_start;
        let slot_index = offset / SLOT_SIZE;

        let used = &mut *self.used_slots.get();
        used[slot_index] = false;
=======
        self.used.set(true);

        // Get a raw pointer to the start of the heap
        (*self.heap.get()).as_mut_ptr()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        self.used.set(false);
>>>>>>> 01a949feee4aae2cf6186c16b74725448c062c13
    }
}

#[global_allocator]
static ALLOCATOR: MyAllocator = MyAllocator {
<<<<<<< HEAD
    heap: UnsafeCell::new([0; HEAP_SIZE]),
    used_slots: UnsafeCell::new([false; NUM_SLOTS]),
=======
    heap: UnsafeCell::new([0; 1024]),
    used: Cell::new(false),
>>>>>>> 01a949feee4aae2cf6186c16b74725448c062c13
};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
