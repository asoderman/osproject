#![no_std]
#![feature(exclusive_range_pattern)]
#![feature(abi_x86_interrupt)]
#![feature(allocator_api)]
#![feature(const_fn)]
#![feature(alloc_error_handler)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(naked_functions)]

#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(box_syntax)]
#![feature(llvm_asm)]

extern crate alloc;

pub mod interrupt;
pub mod vga_text_buffer;
pub mod gdt;
pub mod memory;
pub mod task;
pub mod rtc;
pub mod debug;

//pub mod machine;
//pub mod thread;

pub mod context;
pub mod proc;


use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;


use core::panic::PanicInfo;

#[cfg(test)]
use bootloader::{BootInfo, entry_point};

#[cfg(test)]
entry_point!(test_kernel_main);

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/*
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
*/

use core::cell::UnsafeCell;

static mut TSS: UnsafeCell<Option<TaskStateSegment>> = UnsafeCell::new(None);

pub fn get_tss() -> &'static TaskStateSegment {
    unsafe {
        if let Some(_tss) = *TSS.get() {
            return TSS.get().as_ref().unwrap().as_ref().unwrap();
        } else {
            let new = Some(create_tss());
            let ptr = TSS.get();
            *ptr = new;
            return get_tss();
        }
    }
}


fn create_tss() -> TaskStateSegment {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = 4096;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

        let stack_start = VirtAddr::from_ptr(unsafe {&STACK});
        let stack_end = stack_start + STACK_SIZE;
        stack_end
    };
    tss
}

pub fn update_TSS(rsp: usize) {
    // Update ring 0 stack
    unsafe {
        use core::convert::TryInto;
        if let Some(mut tss) = *TSS.get() {
            tss.privilege_stack_table[0] = VirtAddr::new(rsp.try_into().unwrap());
        }
    }
}

pub fn init() {
    gdt::init();
    interrupt::init_idt();
    unsafe { interrupt::PICS.lock().initialize() };
    //x86_64::instructions::interrupts::enable();
}

pub fn enable_interrupts() {
    unsafe {
        llvm_asm!("sti");
    }
}

pub fn halt_loop() -> ! {
    enable_interrupts();
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
    dbg_println!("Exiting...");
    exit_qemu(QemuExitCode::Success);
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    dbg_println!("Running {} tests", tests.len());
    for test in tests {
        test();
        dbg_print!(".");
    }
    dbg_println!("\n\x1b[32m[ok]\x1b[0m");
    exit();
}

#[cfg(test)]
pub static mut PHYS_OFFSET: u64 = 0;

#[cfg(test)]
pub fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
    init();
    let phys_mem_offset = VirtAddr::new(_boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::paging::offset_page_table(phys_mem_offset) };

    let mut frame_allocator = unsafe { 
        memory::BootInfoFrameAllocator::init(&_boot_info.memory_map)
    };

    memory::allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("Heap initialization failed");
    // using a global variable for testing purposes only
    unsafe {
        PHYS_OFFSET = _boot_info.physical_memory_offset; 
    }
    test_main();
    halt_loop();
}


pub fn test_panic_handler(info: &PanicInfo) -> ! {
    dbg_println!("\x1b[0;31;m[failed]\x1b[0m \n");
    dbg_println!("Error: {}", info);
    dbg_println!("Invoked from: {}", core::file!());
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
}
