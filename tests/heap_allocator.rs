#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_esgi::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use rust_esgi::allocator::HEAP_SIZE;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    use rust_esgi::allocator;
    use rust_esgi::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    rust_esgi::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    test_main();
    rust_esgi::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_esgi::test_panic_handler(info)
}

#[test_case]
fn test_simple_box_allocation() {
    let heap_value = Box::new(42_u32);
    assert_eq!(*heap_value, 42);
}

#[test_case]
fn test_many_boxes() {
    for i in 0..1_000_u64 {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}

#[test_case]
fn test_many_boxes_long_lived() {
    let mut boxes = Vec::new();
    for i in 0..1_000_u64 {
        boxes.push(Box::new(i));
    }
    for (i, b) in boxes.iter().enumerate() {
        assert_eq!(**b, i as u64);
    }
}

#[test_case]
fn test_vec_push_pop() {
    let mut v: Vec<u32> = Vec::new();
    for i in 0..256_u32 {
        v.push(i * 2);
    }
    assert_eq!(v.len(), 256);
    for i in 0..256_u32 {
        assert_eq!(v[i as usize], i * 2);
    }
}

#[test_case]
fn test_large_allocation() {
    let v: Vec<u8> = vec![0xAB_u8; 4096];
    assert_eq!(v.len(), 4096);
    assert!(v.iter().all(|&b| b == 0xAB));
}

#[test_case]
fn test_all_size_classes() {
    use rust_esgi::allocator::slab::SLAB_SIZES;

    let mut pointers: Vec<Box<[u8]>> = Vec::new();
    for &sz in SLAB_SIZES {
        let boxed: Box<[u8]> = vec![0_u8; sz].into_boxed_slice();
        pointers.push(boxed);
    }

    for i in 0..pointers.len() {
        for j in (i + 1)..pointers.len() {
            assert_ne!(
                pointers[i].as_ptr() as usize,
                pointers[j].as_ptr() as usize,
                "Deux allocations ont retourné la même adresse !"
            );
        }
    }
}

#[test_case]
fn test_allocate_free_cycle() {
    for i in 0..10_000_u64 {
        let b = Box::new(i);
        assert_eq!(*b, i);
    }
}

#[test_case]
fn test_rc_allocation() {
    let a = Rc::new(99_u32);
    let b = Rc::clone(&a);
    assert_eq!(*a, 99);
    assert_eq!(*b, 99);
    assert_eq!(Rc::strong_count(&a), 2);
}

#[test_case]
fn test_heap_stress() {
    const N_ITEMS: usize = (HEAP_SIZE / 8) * 9 / 10;
    let mut v: Vec<u64> = Vec::with_capacity(N_ITEMS);
    for i in 0..N_ITEMS as u64 {
        v.push(i);
    }
    for (i, &val) in v.iter().enumerate() {
        assert_eq!(val, i as u64);
    }
}
