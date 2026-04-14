#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
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

unsafe impl Sync for MyAllocator {}

unsafe impl GlobalAlloc for MyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > SLOT_SIZE {
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
    }
}

#[global_allocator]
static ALLOCATOR: MyAllocator = MyAllocator {
    heap: UnsafeCell::new([0; HEAP_SIZE]),
    used_slots: UnsafeCell::new([false; NUM_SLOTS]),
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
