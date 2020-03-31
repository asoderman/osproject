#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(oslib::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;
use bootloader::{BootInfo, entry_point};

use oslib::{init, println, dbg_println, halt_loop, task::TaskManager};

#[cfg(not(test))]
entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use oslib::memory;
    use x86_64::VirtAddr;

    dbg_println!("Booting os...");
    println!("Hello world{}", "!");

    // TODO: This probably needs to be towards the end of main
    // since interrupts become enabled we don't know if anything else
    // will run.
    init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

    let frame_allocator = unsafe { 
        memory::BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    let mut mapper = unsafe { memory::paging::offset_page_table(phys_mem_offset) };
    let mut memory_manager = memory::paging::MemoryManager::new(frame_allocator);

    let heap_region = memory::allocator::init_heap(&mut mapper, &mut memory_manager.frame_allocator)
        .expect("Heap initialization failed");
    memory_manager.heap_was_init_at(heap_region);
    let test_addr = VirtAddr::new(0x0f00000000);
    use x86_64::structures::paging::mapper::MapperAllSizes;
    memory_manager.request_address_space_at(test_addr, 5 * 1024, &mut mapper);
    dbg_println!("Requested address space from memory manager. Page should be mapped");
    dbg_println!("{:?} -> {:?}", test_addr, mapper.translate(test_addr));
    dbg_println!("Memory regions: {:?}", memory_manager.get_used_regions());
    dbg_println!("");
    dbg_println!("Relinquishing address space");
    memory_manager.relinquish_address_space(test_addr, 5 * 1024, &mut mapper);
    dbg_println!("Memory regions: {:?}", memory_manager.get_used_regions());
    dbg_println!("");
    memory_manager.request_address_space_at(test_addr, 1024, &mut mapper);
    dbg_println!("{:?} -> {:?}", test_addr, mapper.translate(test_addr));
    dbg_println!("Memory regions: {:?}", memory_manager.get_used_regions());


    dbg_println!("Initializing task manager");
    TaskManager::new();
    dbg_println!("TaskManager initialized");

    #[cfg(test)]
    test_main();

    halt_loop();
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
