#![no_std]
#![feature(exclusive_range_pattern)]
#![feature(abi_x86_interrupt)]
#![feature(allocator_api)]
#![feature(const_fn)]
#![feature(alloc_error_handler)]
#![feature(asm)]

#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

pub mod interrupt;
pub mod vga_text_buffer;
pub mod gdt;
pub mod memory;
pub mod task;
pub mod debug;

use spin::Mutex;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use lazy_static::lazy_static;

use core::panic::PanicInfo;

#[cfg(test)]
use bootloader::{BootInfo, entry_point};

#[cfg(test)]
entry_point!(test_kernel_main);

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe {&STACK});
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}

pub fn init() {
    gdt::init();
    interrupt::init_idt();
    unsafe { interrupt::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
}

pub fn halt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

pub fn exit() {
    exit_qemu(QemuExitCode::Success);
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    dbg_println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }

    exit();
}

#[cfg(test)]
pub fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
    init();
    test_main();
    halt_loop();
}


pub fn test_panic_handler(info: &PanicInfo) -> ! {
    dbg_println!("[failed] \n");
    dbg_println!("Error: {}", info);
    exit_qemu(QemuExitCode::Failed);
    halt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}


#[test_case]
fn trivial_assert() {
    assert_eq!(1, 1);
    dbg_println!("[ok]");
    exit();
}
