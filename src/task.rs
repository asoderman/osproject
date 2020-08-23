use alloc::{boxed::Box, vec, collections::BTreeMap};
use core::sync::atomic::AtomicBool;

use core::mem;
use core::alloc::{GlobalAlloc, Layout};

use alloc::sync::Arc;
use spin::{Mutex, RwLock};

use lazy_static::lazy_static;

pub static CONTEXT_SWITCH_LOCK: AtomicBool = AtomicBool::new(false);

// FIXME: Arbitrary number of processes
pub static MAX_PROCS: usize = 256;

// FIXME: rough draft error codes
pub static EAGAIN: i32 = -2;


lazy_static! {

    pub static ref TASKMANAGER: Mutex<TaskManager> = Mutex::new(TaskManager::new());

}


pub struct TaskManager {

    pub procs: BTreeMap<usize, Arc::<RwLock<Proc>>>,
    next_id: usize

    //current_task: usize,
    //num_tasks: usize

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
            self.procs.insert(self.next_id, Arc::new(RwLock::new(p)));
            self.next_id += 1;

            Ok(self.procs
               .get(&(self.next_id - 1))
               .expect("Failed to create new process"))

    }

    pub fn remove(&mut self, id: usize) -> Option<Arc<RwLock<Proc>>> {
        self.procs.remove(&id)
    }

    pub fn spawn(&mut self, func: extern fn()) -> 
        Result<&Arc<RwLock<Proc>>, i32> {
            let proc_lock = self.new_proc()?;
            {
                let mut proc = proc_lock.write();
                let mut fx = unsafe { Box::from_raw(crate::memory::allocator::ALLOCATOR.alloc(Layout::from_size_align_unchecked(512, 16)) as *mut [u8; 512]) };

                for b in fx.iter_mut() {
                    *b = 0;
                }

                let mut stack = vec![0; 4096].into_boxed_slice();

                let offset = stack.len() - mem::size_of::<usize>();

                unsafe {
                    let offset = stack.len() - mem::size_of::<usize>();
                    let func_ptr = stack.as_mut_ptr().add(offset);
                    *(func_ptr as *mut usize) = func as usize;
                }

                //proc.cpu_context.set_page_table( 

                proc.cpu_context.set_fx(fx.as_ptr() as usize);
                proc.cpu_context.set_stack(stack.as_ptr() as usize + offset);
                proc.kfx = Some(fx);
                proc.kstack = Some(stack);
            }
        Ok(proc_lock)
    }
}

// Rudimentary process structure
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

    pub fn switch_to(&mut self, other: &mut Self) {
        x86_64::instructions::interrupts::disable();
        unsafe {
            self.cpu_context.switch(&mut other.cpu_context);
        }
    }

    pub fn dump(&self) {
        crate::dbg_println!("{} STACK DUMP: {:?}", self.id, self.kstack);
    }
}

pub fn spawn_kernel_task(f: extern fn()) {
    let _ = TASKMANAGER.lock().spawn(f);
}

pub fn get_procs() -> usize {
    TASKMANAGER.lock().procs.len()
}

// The state of the cpu during execution
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

    pub fn set_fx(&mut self, address: usize) {
        self.fx = address;
    }

    pub unsafe fn push_stack(&mut self, value: usize) {
        self.rsp -= mem::size_of::<usize>();
        *(self.rsp as *mut usize) = value;
    }

    pub unsafe fn pop_stack(&mut self) -> usize {
        let value = *(self.rsp as *mut usize);
        self.rsp += mem::size_of::<usize>();
        value
    }

    pub unsafe fn switch(&mut self, next: &mut CPUContext) {
        crate::dbg_println!("switching contexts");
        // Save the floating point register
        llvm_asm!("fxsave64 [$0]" : :"r"(self.fx) : "memory": "intel", "volatile");
        self.loadable = true;

        if next.loadable {
            llvm_asm!("fxrstor64 [$0]" : : "r"(next.fx) : "memory" : 
                 "intel", "volatile");
        } else { 
            llvm_asm!("fninit": : : "memory" :
                 "intel", "volatile");
        }

        // move the current cr3 (page table address) into the structure
        //asm!("mov $0, cr3": : "r"(self.cr3) : :"memory" : 
        //    "intel", "volatile");
        // check if the cr3 needs to be updated 
        if next.cr3 != self.cr3 {
            llvm_asm!("mov cr3, $0" : : "r"(next.cr3) : "memory" : 
                 "intel", "volatile");
        }

        // preserve then update the CPU registers
        llvm_asm!("pushfq ; pop $0" : "=r"(self.rflags) : : 
             "memory" : "intel", "volatile");
        llvm_asm!("push $0 ; popfq" : : "r"(next.rflags) : 
             "memory" : "intel", "volatile");

        llvm_asm!("mov $0, rbx" : "=r"(self.rbx) : : "memory" : 
             "intel", "volatile");
        llvm_asm!("mov rbx, $0" : : "r"(next.rbx) : "memory" : 
             "intel", "volatile");

        llvm_asm!("mov $0, r12" : "=r"(self.r12) : : "memory" : 
             "intel", "volatile");
        llvm_asm!("mov r12, $0" : : "r"(next.r12) : "memory" : 
             "intel", "volatile");

        llvm_asm!("mov $0, r13" : "=r"(self.r13) : : "memory" : 
             "intel", "volatile");
        llvm_asm!("mov r13, $0" : : "r"(next.r13) : "memory" : 
             "intel", "volatile");

        llvm_asm!("mov $0, r14" : "=r"(self.r14) : : "memory" : 
             "intel", "volatile");
        llvm_asm!("mov r14, $0" : : "r"(next.r14) : "memory" : 
             "intel", "volatile");

        llvm_asm!("mov $0, r15" : "=r"(self.r15) : : "memory" : 
             "intel", "volatile");
        llvm_asm!("mov r15, $0" : : "r"(next.r15) : "memory" : 
             "intel", "volatile");

        llvm_asm!("mov $0, rsp" : "=r"(self.rsp) : : "memory" : 
             "intel", "volatile");
        llvm_asm!("mov rsp, $0" : : "r"(next.rsp) : "memory" : 
             "intel", "volatile");

        llvm_asm!("mov $0, rbp" : "=r"(self.rbp) : : "memory" : 
             "intel", "volatile");
        llvm_asm!("mov rbp, $0" : : "r"(next.rbp) : "memory" : 
             "intel", "volatile");

    }
}
