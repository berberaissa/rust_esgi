//! Point d'entrée du noyau `rust_esgi`.
//!
//! `kernel_main` est appelé par le bootloader après avoir mappé la
//! mémoire physique et construit la `BootInfo`.
#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_esgi::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use rust_esgi::{println, serial_println};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use rust_esgi::allocator;
    use rust_esgi::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    serial_println!("rust_esgi kernel starting...");
    println!("rust_esgi kernel starting...");

    rust_esgi::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };

    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    let heap_value = Box::new(41);
    serial_println!("heap_value = {}", heap_value);
    println!("heap_value = {}", heap_value);

    let mut vec: Vec<u32> = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    serial_println!("vec[100] = {}, len = {}", vec[100], vec.len());
    println!("vec[100] = {}, len = {}", vec[100], vec.len());

    let rc_a = Rc::new(100u32);
    let rc_b = Rc::clone(&rc_a);
    serial_println!("Rc strong count = {}", Rc::strong_count(&rc_a));
    println!("Rc strong count = {}", Rc::strong_count(&rc_a));
    drop(rc_b);
    serial_println!("Rc strong count after drop = {}", Rc::strong_count(&rc_a));
    println!("Rc strong count after drop = {}", Rc::strong_count(&rc_a));

    serial_println!("Tous les tests heap ont reussi !");
    println!("Tous les tests heap ont reussi !");

    #[cfg(test)]
    test_main();

    rust_esgi::hlt_loop();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("{}", info);
    println!("{}", info);
    rust_esgi::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_esgi::test_panic_handler(info)
}
