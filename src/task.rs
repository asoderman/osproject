use alloc::{boxed::Box, vec, vec::Vec, collections::BTreeMap};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::dbg_println;

use core::mem;
use core::alloc::{GlobalAlloc, Layout};

use alloc::sync::{Arc, Weak};
use spin::{RwLock, Mutex};
use lazy_static::lazy_static;

pub static CONTEXT_SWITCH_LOCK: AtomicBool = AtomicBool::new(false);

// FIXME: Arbitrary number of processes
pub static MAX_PROCS: usize = (isize::max_value() as usize) - 1;

// FIXME: rough draft error codes
pub static EAGAIN: i32 = -2;

// FIXME: probably needs to be a rwlock instead of mutex
lazy_static!{
    pub static ref TASKS: Mutex<TaskManager> = Mutex::new(TaskManager::new());
}

pub static CURRENT_PROC: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    // Create the first process

    crate::dbg_println!("Initializing process system. Creating the first Proc");
    let mut tasks =  TASKS.lock();
    let proc_locked = tasks.new_proc().expect("Could not create first proc");
    let mut proc = proc_locked.write();

    let mut fx = unsafe { Box::from_raw(crate::memory::allocator::ALLOCATOR.alloc(Layout::from_size_align_unchecked(512, 16)) as *mut [u8; 512]) };

    for b in fx.iter_mut() {
        *b = 0;
    }

    proc.cpu_context.set_fx(fx.as_ptr() as usize);
    proc.kfx = Some(fx);
    proc.running = false;

    CURRENT_PROC.store(proc.id, Ordering::SeqCst);

    crate::dbg_println!("First process created with id: {}", proc.id);
}

pub fn switch() {

    crate::dbg_println!("switch(): entered function");

    while CONTEXT_SWITCH_LOCK.compare_and_swap(false, true, Ordering::SeqCst) {
        x86_64::instructions::interrupts::disable();
    }

    dbg_println!("Interrupts disabled");
    // If there are no other tasks to switch to do nothing.
    let tasks = TASKS.lock();
    dbg_println!("Acquired global task list lock");
    dbg_println!("procs count: {}", tasks.procs.len());
    if (tasks.procs.is_empty() || tasks.procs.len() == 1) {
        crate::dbg_println!("Switch(): doing nothing - early return");
        return
    }

    dbg_println!("procs list isnt empty");

    // tasks lock must be dropped otherwise current_proc() will hang
    drop(tasks);

    let current = current_proc();
    dbg_println!("retrieved current proc");
    let mut current_id = current.read().get_id();
    let next: Weak<RwLock<Proc>>;


    loop {
        dbg_println!("Looping...");
        if current_id >= TASKS.lock().next_id {
            next = Arc::downgrade(&TASKS.lock().get_runnable().next().expect("Could not get a new runnable context in switch()").1);
            break;
        } else {
            current_id += 1;
            if let Some(new) = TASKS.lock().procs.get(&current_id) {
                if !(new.read().running) {
                    next = Arc::downgrade(&new);
                    break;
                }
            }
        }
    }

    dbg_println!("Exited the schedule loop");
    dbg_println!("Current id: {}", current_id);

    // Do proc swap and drop locks
    let current_weak = Arc::downgrade(&current);
    drop(current);
    dbg_println!("Storing proc id");
    CURRENT_PROC.store(next.upgrade().expect("Proc did not live long enough in proc switch function").read().get_id(), Ordering::SeqCst);

    let ticks = crate::interrupt::PIT_TICKS.swap(0, Ordering::SeqCst);

    CONTEXT_SWITCH_LOCK.store(false, Ordering::SeqCst);

    unsafe {
        if let Some(ref stack) = next.upgrade().unwrap().read().kstack {
            // swap the stacks
            crate::gdt::set_tss(stack.as_ptr() as usize + stack.len());
        }
    }

    // FIXME: instuction pointer getting set to virt addr 0 after 
    // context swithc. Check if stack is being 0'd out and potentially 
    // jumping to one of the values
    if next.as_raw() as usize != 0 {
        // I am not sure if these locks are released 
        unsafe {
            (*current_weak.as_raw()).write().cpu_context.switch(&mut (*next.as_raw()).write().cpu_context);
        }
    }

}

// TODO: Should probably return a Weak reference 
pub fn current_proc() -> Arc<RwLock<Proc>> {
    TASKS.lock().procs.get(&CURRENT_PROC.load(Ordering::SeqCst)).expect("current_proc() called before any process has been created").clone()
}

pub struct TaskManager {
    procs: BTreeMap<usize, Arc::<RwLock<Proc>>>,
    next_id: usize
}

impl TaskManager {

    pub fn new() -> TaskManager {
        TaskManager {
            procs: BTreeMap::new(),
            next_id: 1
        }
    }
    
    pub fn new_proc(&mut self) -> 
        Result<&Arc<RwLock<Proc>>, i32> {
            if self.next_id >= MAX_PROCS { 
                self.next_id = 1;
            }

            while self.procs.contains_key(&self.next_id) {
                self.next_id += 1;
            }

            if self.next_id >= MAX_PROCS {
                return Err(EAGAIN);
            }

            let p = Proc::from(self.next_id);
            self.next_id += 1;
            let id = p.get_id();
            assert!(self.procs.insert(p.get_id(), Arc::new(RwLock::new(p))).is_none());
            Ok(self.procs
               .get(&id)
               .expect("Failed to create new process"))

    }

    pub fn remove(&mut self, id: usize) -> Option<Arc<RwLock<Proc>>> {
        self.procs.remove(&id)
    }

    // TODO: status enum instead of bool
    pub fn get_runnable(&self) -> impl Iterator<Item = (&usize, &Arc<RwLock<Proc>>)> {
        self.procs.iter().filter(|(k, v)| { 
            !v.clone().read().running 
        })
    }

    pub fn spawn(&mut self, func: extern fn()) -> 
        Result<&Arc<RwLock<Proc>>, i32> {
            let proc_lock = self.new_proc()?;
            {
                crate::dbg_println!("Spawning new process...");
                let mut proc = proc_lock.write();
                let mut fx = unsafe { Box::from_raw(crate::memory::allocator::ALLOCATOR.alloc(Layout::from_size_align_unchecked(512, 16)) as *mut [u8; 512]) };

                for b in fx.iter_mut() {
                    *b = 0;
                }

                let mut stack = vec![0; 65_536].into_boxed_slice();
                let offset = stack.len() - mem::size_of::<usize>();

                unsafe {
                    let offset = stack.len() - mem::size_of::<usize>();
                    let func_ptr = stack.as_mut_ptr().add(offset);
                    *(func_ptr as *mut usize) = func as usize;
                }

                let current_page_table = x86_64::registers::control::Cr3::read().0.start_address();

                proc.cpu_context.set_page_table(current_page_table.as_u64() as usize);

                proc.cpu_context.set_fx(fx.as_ptr() as usize);
                proc.cpu_context.set_stack(stack.as_ptr() as usize + offset);
                proc.kfx = Some(fx);
                proc.kstack = Some(stack);
            }
        Ok(proc_lock)
    }
}

// Rudimentary process structure
#[derive(Debug, Clone)]
pub struct Proc {
    pub id: usize,
    pub running: bool,
    pub cpu_context: CPUContext,

    pub kfx: Option<Box<[u8]>>,
    pub kstack: Option<Box<[u8]>>

}

impl Proc {

    pub fn from(id: usize) -> Self {
        Proc {
            id,
            running: false,
            cpu_context: CPUContext::new(),
            kfx: None,
            kstack: None,
        }
    }

    pub fn get_id(&self) -> usize {
        self.id
    }
}

// The state of the cpu during execution. x86_64 arch
#[derive(Clone, Debug)]
pub struct CPUContext {

    loadable: bool,

    fx: usize,
    cr3: usize, 
    rflags: usize,
    rbx: usize, 
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,

    rbp: usize,
    rsp: usize,

}

impl CPUContext {
    pub fn new() -> Self {
        CPUContext {
            loadable: false,

            fx: 0,
            cr3: 0,
            rflags: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,

            rbp: 0,
            rsp: 0,
        }
    }

    pub fn get_page_table(&mut self) -> usize {
        self.cr3
    }

    pub fn set_page_table(&mut self, address: usize) {
        self.cr3 = address;
    }

    pub fn set_stack(&mut self, address: usize) {
        self.rsp = address;
    }

    pub fn push_stack(&mut self, value: usize) {
        self.rsp -= mem::size_of::<usize>();
        //*(self.rsp as *mut usize) = value;
    }

    /* pub fn pop_stack(&mut self) -> usize {
        let value = *(self.rsp as *const usize);
        self.rsp += mem::size_of::<usize>();
        value
    } */

    pub fn set_fx(&mut self, address: usize) {
        self.fx = address;
    }

    pub unsafe fn switch(&mut self, next: &mut CPUContext) {
        // Save the floating point register
        crate::dbg_println!("Switching cpu context");
        asm!("fxsave64 [$0]" : :"r"(self.fx) : "memory": "intel", "volatile");
        self.loadable = true;

        if next.loadable {
            asm!("fxrstor64 [$0]" : : "r"(next.fx) : "memory" : 
                 "intel", "volatile");
        } else { 
            asm!("fninit": : : "memory" :
                 "intel", "volatile");
        }

        // move the current cr3 (page table address) into the structure
        asm!("mov $0, cr3": "=r"(self.cr3) : :"memory" : 
             "intel", "volatile");
        // check if the cr3 needs to be updated 
        if next.cr3 != self.cr3 {
            asm!("mov cr3, $0" : : "r"(next.cr3) : "memory" : 
                 "intel", "volatile");
        }

        // preserve then update the CPU registers
        asm!("pushfq ; pop $0" : "=r"(self.rflags) : : 
             "memory" : "intel", "volatile");
        asm!("push $0 ; popfq" : : "r"(next.rflags) : 
             "memory" : "intel", "volatile");

        asm!("mov $0, rbx" : "=r"(self.rbx) : : "memory" : 
             "intel", "volatile");
        asm!("mov rbx, $0" : : "r"(next.rbx) : "memory" : 
             "intel", "volatile");

        asm!("mov $0, r12" : "=r"(self.r12) : : "memory" : 
             "intel", "volatile");
        asm!("mov r12, $0" : : "r"(next.r12) : "memory" : 
             "intel", "volatile");

        asm!("mov $0, r13" : "=r"(self.r13) : : "memory" : 
             "intel", "volatile");
        asm!("mov r13, $0" : : "r"(next.r13) : "memory" : 
             "intel", "volatile");

        asm!("mov $0, r14" : "=r"(self.r14) : : "memory" : 
             "intel", "volatile");
        asm!("mov r14, $0" : : "r"(next.r14) : "memory" : 
             "intel", "volatile");

        asm!("mov $0, r15" : "=r"(self.r15) : : "memory" : 
             "intel", "volatile");
        asm!("mov r15, $0" : : "r"(next.r15) : "memory" : 
             "intel", "volatile");

        asm!("mov $0, rsp" : "=r"(self.rsp) : : "memory" : 
             "intel", "volatile");
        asm!("mov rsp, $0" : : "r"(next.rsp) : "memory" : 
             "intel", "volatile");

        asm!("mov $0, rbp" : "=r"(self.rbp) : : "memory" : 
             "intel", "volatile");
        asm!("mov rbp, $0" : : "r"(next.rbp) : "memory" : 
             "intel", "volatile");

    }
}
