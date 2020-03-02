use alloc::alloc::{GlobalAlloc, Layout, AllocErr};
use core::ptr::NonNull;

use super::list_allocator::{ListHeap, HoleInfo};

use lazy_static::lazy_static;

use spin::Mutex;

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB
    },
    VirtAddr,
};

// TODO: implement Slab allocator and possibly abstract out a heap interface

#[global_allocator]
pub static ALLOCATOR: LockedListHeap = LockedListHeap::empty();

// TODO: Find appropriate values for these
pub const HEAP_START: usize = 0x4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024;

pub fn init_heap(mapper: &mut impl Mapper<Size4KiB>,
                 frame_allocator: &mut impl FrameAllocator<Size4KiB>
                 ) -> Result<(), MapToError> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator.allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        mapper.map_to(page, frame, flags, frame_allocator)?.flush();
    }

    unsafe {
        ALLOCATOR.0.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())

}

pub trait Heap {
    // General trait for allocators 
    // TODO: once traits support const fn empty() -> Self where Self: Heap;
    fn init(&mut self, heap_bottom: usize, heap_size: usize);
    fn new(heap_bottom: usize, heap_size: usize) -> Self where Self: Heap;
    fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, AllocErr>;
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout);

}

pub struct Allocation {
    // TODO: abstract HoleInfo away to become more general to support
    // other allocators
    pub info: HoleInfo,
    pub front_padding: Option<HoleInfo>,
    pub back_padding: Option<HoleInfo>
}

// TODO: unable to fully abstract away locked heaps due to const functions
// not being supported within traits
pub struct LockedHeap<T: Heap>(Mutex<T>);

impl <T: Heap> LockedHeap<T> {

    pub const fn empty_from_heap(heap: T) -> LockedHeap<T> {
        LockedHeap(Mutex::new(heap))
    }

    pub unsafe fn new(heap_bottom: usize, heap_size: usize) -> LockedHeap<T> {
        LockedHeap(Mutex::new( T::new(heap_bottom, heap_size)))
    }
}

unsafe impl <T: Heap> GlobalAlloc for LockedHeap<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.lock().allocate(layout).ok().map_or(0 as *mut u8, |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.lock().deallocate(NonNull::new_unchecked(ptr), layout);
    }
}

pub struct LockedListHeap(Mutex<ListHeap>);

impl LockedListHeap {
    const fn empty() -> LockedListHeap {
        LockedListHeap(Mutex::new(ListHeap::empty()))
    }

    pub unsafe fn new(heap_bottom: usize, heap_size: usize) -> LockedListHeap{
        LockedListHeap(Mutex::new(ListHeap::new(heap_bottom, heap_size)))
    }
}

unsafe impl GlobalAlloc for LockedListHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.lock().allocate(layout).ok().map_or(0 as *mut u8, |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.lock().deallocate(NonNull::new_unchecked(ptr), layout);
    }
}


fn align_down(addr: usize, align: usize) -> usize {
    if align.is_power_of_two() {
        addr & !(align - 1)
    } else if align == 0 {
        addr
    } else {
        panic!("align must be a power of 2");
    }
}

pub fn align_up(addr: usize, align: usize) -> usize {
    align_down(addr + align - 1, align)
}

pub fn move_helper<T>(x: T) -> T {
    x
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("Allocation error {:?}", layout);
}
