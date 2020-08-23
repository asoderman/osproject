use crate::context::Stack;

use core::sync::atomic::AtomicUsize;

use lazy_static::lazy_static;
use spin::Mutex;

use alloc::collections::VecDeque;

static mut NEXT_PID: AtomicUsize = AtomicUsize::new(0);

pub static mut TICKS: AtomicUsize = AtomicUsize::new(0);

lazy_static! {
    pub static ref READY: Mutex<VecDeque<Process>> = Mutex::new(VecDeque::new());
}

lazy_static! {
    static ref ACTIVE: Mutex<Option<Process>> = Mutex::new(None);
}

fn new_pid() -> usize {
    // FIXME: Doesn't handle overflow, does not account for used/free pids
    unsafe {
        let mut p = NEXT_PID.get_mut();
        *p += 1;
        *p
    }
}

#[derive(Clone, Debug)]
enum Priority {
    High,
    Low
}

#[derive(Clone, Debug)]
pub struct Process {
    id: usize,
    context: Stack,

    priority: Priority
}

impl Process {

    pub fn new() -> Process {
        Process {
            id: new_pid(),
            context: Stack::new(),

            priority: Priority::Low,
        }
    }

    pub fn set_entry(&mut self, addr: usize) {
        self.context.entry_at(addr);
    }

    pub fn spawn_kernel_task(addr: usize) -> Process {
        let mut p = Self::new();
        p.context.set_first_rip(addr);
        p
    }

    pub fn get_pid(&self) -> usize {
        self.id
    }

}

pub fn schedule_and_then_switch() {
    crate::dbg_println!("Preparing context switch");
    let p = schedule();
    switch_to(p);
}

pub fn switch_to(p: Process) {
    // FIXME: These clones are expensive and should be replaced
    let mut p = p;

    let mut current = ACTIVE.lock().take().expect("Taking the current active proc");

    ready(current.clone()); // Place it back in the ready queue to run some more

    make_active(p.clone());

    unsafe {
        crate::context::context_switch(&mut current.context.get_info(), &mut p.context.get_info());
    }
}

pub fn make_active(p: Process) {
    unsafe {
        *ACTIVE.lock() = Some(p);
    }
}

pub fn ready(p: Process) {
    unsafe {
        match p.priority {
            Priority::Low => READY.lock().push_back(p),
            Priority::High => READY.lock().push_front(p),
        }
    }
}

pub fn schedule() -> Process {
    READY.lock().pop_front().expect("READY queue empty. No idle process found.")
}

pub fn my_pid() -> usize {
    let p = ACTIVE.lock().take();
    let mut result = 0;

    if let Some(proc) = p {
        result = proc.id;
        make_active(proc);
    }

    return result;
}

pub fn test_proc() {

    let p1 = Process::spawn_kernel_task(hello_world as usize);
    let p2 = Process::spawn_kernel_task(hello_world as usize);

    let mut p3 = Process::spawn_kernel_task(hang as usize);
    make_active(p3);

    ready(p1);
    ready(p2);


    crate::enable_interrupts();

    extern "C" fn hello_world() {
        let mut cnt = 0;
        loop {
            crate::enable_interrupts();
            if cnt > 75_000_000 {
                crate::dbg_println!("<PROCESS {}>: Hello world!", my_pid());
                cnt = 0;
                x86_64::instructions::hlt();
            }
            cnt += 1;
        }
    }

    extern "C" fn hang() { crate::halt_loop(); }

}
