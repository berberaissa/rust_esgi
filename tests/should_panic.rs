//! Test de comportement à la panique.
//!
//! Ce test vérifie que le panic handler fonctionne correctement dans
//! l'environnement noyau. Il est exécuté dans son propre processus QEMU
//! isolé, séparé des tests normaux.
//!
//! Exécuter avec : `cargo test --test should_panic`
#![no_std]
#![no_main]

use core::panic::PanicInfo;
use rust_esgi::{exit_qemu, serial_print, serial_println, QemuExitCode};

/// Point d'entrée du test.
///
/// Provoque volontairement une panique et vérifie qu'elle est bien
/// interceptée par notre panic handler personnalisé.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

/// Cette fonction doit déclencher une panique.
fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

/// En mode `should_panic`, une panique EST le succès.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
