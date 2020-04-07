#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(oslib::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;
use bootloader::{BootInfo, entry_point};

use oslib::{init, println, dbg_println, halt_loop};

#[cfg(not(test))]
entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use oslib::memory;
    use x86_64::VirtAddr;

    // TODO: This probably needs to be towards the end of main
    // since interrupts become enabled we don't know if anything else
    // will run.
    init();

    dbg_println!("Booting os...");
    println!("Hello world{}", "!");


    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

    let frame_allocator = unsafe { 
        memory::BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    let mut mapper = unsafe { memory::paging::offset_page_table(phys_mem_offset) };
    let mut memory_manager = memory::paging::MemoryManager::new(frame_allocator);

    let heap_region = memory::allocator::init_heap(&mut mapper, &mut memory_manager.frame_allocator)
        .expect("Heap initialization failed");
    memory_manager.heap_was_init_at(heap_region);


    dbg_println!("Initializing task manager");
    oslib::task::init();
    oslib::task::TASKS.lock().spawn(hello_world).expect("Could not spawn a new process from function");
    dbg_println!("TaskManager initialized");

    oslib::interrupt::enable_context_switching();
    dbg_println!("Boot time: {}", *oslib::rtc::BOOT_TIME);


    halt_loop();
}

extern "C" fn hello_world() {
    dbg_println!("Hello world (as a process)");
}

#[cfg(not(test))]
#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {

    println!("{}", _info);
    halt_loop();

}

#[cfg(test)]
#[panic_handler]
fn test_panic_handler(_info: &PanicInfo) -> ! {
    oslib::test_panic_handler(_info)
}
