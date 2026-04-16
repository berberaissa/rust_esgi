#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::panic::PanicInfo;
use core::ptr::null_mut;

/// Total memory available for the allocator
const HEAP_SIZE: usize = 1024;

/// Size of each slot (fixed size allocation)
const SLOT_SIZE: usize = 128;

/// Number of slots in the heap
const NUM_SLOTS: usize = HEAP_SIZE / SLOT_SIZE;

/// Simple slab allocator: fixed-size slots
struct MyAllocator {
    /// Raw memory buffer
    heap: UnsafeCell<[u8; HEAP_SIZE]>,

    /// Tracks which slots are used/free
    used_slots: UnsafeCell<[bool; NUM_SLOTS]>,
}

/// Needed because this is a global allocator
unsafe impl Sync for MyAllocator {}

unsafe impl GlobalAlloc for MyAllocator {
    /// Allocate one slot if available
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Too big for a slot → fail
        if layout.size() > SLOT_SIZE {
            return null_mut();
        }

        let used = &mut *self.used_slots.get();
        let heap_ptr = (*self.heap.get()).as_mut_ptr();

        // Find first free slot
        let mut i = 0;
        while i < NUM_SLOTS {
            if !used[i] {
                used[i] = true; // mark as used
                return heap_ptr.add(i * SLOT_SIZE); // return slot pointer
            }
            i += 1;
        }

        // No free slot
        null_mut()
    }

    /// Free the slot corresponding to the pointer
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let heap_start = (*self.heap.get()).as_mut_ptr() as usize;
        let ptr_addr = ptr as usize;

        // Ignore pointers outside our heap
        if ptr_addr < heap_start || ptr_addr >= heap_start + HEAP_SIZE {
            return;
        }

        // Compute which slot this pointer belongs to
        let offset = ptr_addr - heap_start;
        let slot_index = offset / SLOT_SIZE;

        let used = &mut *self.used_slots.get();
        used[slot_index] = false; // mark slot as free
    }
}

/// Register our allocator globally
#[global_allocator]
static ALLOCATOR: MyAllocator = MyAllocator {
    heap: UnsafeCell::new([0; HEAP_SIZE]),
    used_slots: UnsafeCell::new([false; NUM_SLOTS]),
};

/// Required panic handler for no_std
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Exit the program with a status code (used for testing)
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

/// Entry point (no_std version of main)
#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        let layout = Layout::from_size_align(64, 8).unwrap();

        let mut ptrs = [core::ptr::null_mut(); 8];

        // Fill all slots
        let mut i = 0;
        while i < 8 {
            let p = ALLOCATOR.alloc(layout);
            if p.is_null() {
                exit(1); // allocation failed too early
            }
            ptrs[i] = p;
            i += 1;
        }

        // Try one more → should fail
        let extra = ALLOCATOR.alloc(layout);
        if !extra.is_null() {
            exit(2); // allocator didn't stop when full
        }

        // Free one slot
        ALLOCATOR.dealloc(ptrs[0], layout);

        // Allocate again → should work
        let p_new = ALLOCATOR.alloc(layout);
        if p_new.is_null() {
            exit(3); // reuse failed
        }
    }

    exit(0); // everything worked
}
