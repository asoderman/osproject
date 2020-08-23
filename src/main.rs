#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(oslib::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;
extern crate rlibc;

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
    // will run. UPDATE: Moved enable interrupts out of this fn
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
    // Identity map Local APIC
    memory_manager.identity_map(0xfee00000, 1, &mut mapper);
    use x86_64::structures::paging::mapper::MapperAllSizes;


    /* dbg_println!("Initializing task manager");
    TaskManager::new();
    dbg_println!("TaskManager initialized");
    */

    dbg_println!("Boot time: {}", *oslib::rtc::BOOT_TIME);

    //oslib::thread::init();

    use alloc::boxed::Box;
    //oslib::thread::spawn_kernel_task(Box::new(init_task));
    //oslib::thread::spawn_kernel_task(Box::new(dummy_task));

    //dbg_println!("TOTAL PROCS: {}", oslib::task::get_procs());

    //oslib::context::test_context_switch();
    
    oslib::proc::test_proc();

    oslib::enable_interrupts();

    //oslib::thread::cooperative_scheduler_test();

    halt_loop();
}

fn init_task() {

    println!("This is the idle task");

    // modified halt loop
    let mut cnt: usize = 0;
    loop {


        if cnt > 350_000_000 {
            println!("Idle: .");
            cnt = 0;
        }
        cnt += 1;
        //x86_64::instructions::hlt();

    }
    
}

fn dummy_task() {
    println!("Another thread executing.");
    oslib::enable_interrupts();

    loop {
        println!("Another");
        //halt_loop();
        x86_64::instructions::hlt();
    }
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
