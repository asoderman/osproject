
use crate::update_tss;


use alloc::boxed::Box;

global_asm!(include_str!("context.S"));

pub const REGS_TO_SAVE: usize = 7; // Used to calculate offset for stack ptr
const IFRAME_SIZE: usize = 4;

extern "C" {
    pub fn switch_context_inner(current: *mut StackInfo, next: *mut StackInfo);
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct StackInfo {
    stack_ptr: usize
}

impl StackInfo {

    pub fn new(stack_ptr: usize) -> StackInfo {
        StackInfo { stack_ptr }
    }

    pub fn rsp(&self) -> usize {
       self.stack_ptr
    }

}

#[derive(Clone, Debug)]
pub struct Stack{
    stack: Box<[u64]>, 
    rsp: usize,
}

// TODO: Refactor with less hardcoded numbers
impl Stack {
    pub fn new() -> Stack {
        let mut s = Stack { 
            stack: Box::new([0u64; 512]),
            rsp: 510 - REGS_TO_SAVE - IFRAME_SIZE 
        };
        s.entry_at(_first_entry as usize);
        s
    }

    pub fn new_with_start(addr: usize) -> Stack {
        let mut s = Self::new();
        s.set_first_rip(addr);
        s
    }

    pub fn get_info(&mut self) -> StackInfo {
        let offset = self.rsp * core::mem::size_of::<u64>();
        StackInfo::new(self.stack.as_mut_ptr() as usize + offset)
    }

    pub fn entry_at(&mut self, addr: usize) {
        self.stack[510 - IFRAME_SIZE] = addr as u64;
    }

    pub fn has_entry(&self) -> bool {
        self.stack[510 - IFRAME_SIZE] != 0
    }

    pub fn push(&mut self, value: u64) {
        self.rsp -= 1;
        self.stack[self.rsp] = value;
    }

    pub fn pop(&mut self) -> u64 {
        let result = self.stack[self.rsp];
        self.rsp += 1;
        result
    }

    pub fn set_first_rip(&mut self, addr: usize) {
        self.stack[510 - IFRAME_SIZE] = addr as u64;
    }
}

#[cold]
#[inline(never)]
pub unsafe fn context_switch(from: &mut StackInfo, to: &mut StackInfo) {
    update_tss(to.rsp());
    switch_context_inner(from, to);
}

#[no_mangle]
extern "C" fn _first_entry() {
    // Check iframe then iret
    // We need to be careful to not pollute the stack here.
    crate::dbg_println!("Entered _first_entry()");

    unsafe {
        llvm_asm!("iret");
    }
}

pub fn test_context_switch() {

    let mut t1 = Stack::new();
    t1.entry_at(task1_func as usize);
    let mut t2 = Stack::new();
    t2.entry_at(task1_func as usize);

    unsafe {
        context_switch(&mut t1.get_info(), &mut t2.get_info());
    }

    extern "C" fn task1_func() {
        crate::dbg_println!("TASK: Hello world!");
        crate::dbg_println!("Context switch successful");
    }
}

/*
#[derive(Clone, Debug)]
#[repr(C)]
pub struct Context {

    fx: usize,

    rflags: usize,

    cr3: usize, 
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,
    rbx: usize,

    rbp: usize,
    rsp: usize,

}


impl Context {

    pub fn new() -> Context {
        Context {
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

    #[cold]
    #[inline(never)]
    #[naked]
    pub unsafe fn switch_to(&mut self, next: &mut Context) {
        //asm!("fxsave64 [{}]", in(reg), self.fx);
    }
}

*/
