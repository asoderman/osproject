#![no_std]
#![feature(exclusive_range_pattern)]
#![feature(abi_x86_interrupt)]
#![feature(allocator_api)]
#![feature(const_fn)]
#![feature(alloc_error_handler)]
#![feature(asm)]

extern crate alloc;

pub mod interrupt;
pub mod vga_text_buffer;
pub mod gdt;
pub mod memory;
pub mod task;

use spin::Mutex;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use lazy_static::lazy_static;
use uart_16550::SerialPort;

lazy_static! {
    pub static ref SERIAL: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}

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

pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL.lock().write_fmt(args).expect("Printing to serial failed");
    });
}

#[macro_export]
macro_rules! dbg_print {
    ($($arg:tt)*) => {
        $crate::_print(format_args!($($arg)*));
    }
}

#[macro_export]
macro_rules! dbg_println {
    () => ($crate::dbg_print!("\n"));
    ($fmt:expr) => ($crate::dbg_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::dbg_print!(
                concat!($fmt, "\n"), $($arg)*));
}

