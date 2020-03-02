use super::allocator::{Allocation, align_up, Heap, 
    move_helper};
use alloc::alloc::{Layout, AllocErr};
use core::ptr::NonNull;
use core::mem::{align_of, size_of};

pub struct ListHeap {
    bottom: usize,
    size: usize,
    holes: HoleList,
}

impl ListHeap {
    pub fn allocate_first_fit(&mut self, layout: Layout) -> 
        Result<NonNull<u8>, AllocErr> {
            let mut size = layout.size();
            if size < HoleList::min_size() {
                size = HoleList::min_size();
            }

            let size = align_up(size, align_of::<Hole>());
            let layout = Layout::from_size_align(size, layout.align()).unwrap();

            self.holes.allocate_first_fit(layout)
    }
}

impl ListHeap { 
    pub const fn empty()-> ListHeap {
        ListHeap {
            bottom: 0,
            size: 0,
            holes: HoleList::empty()
        }
    }
}

impl Heap for ListHeap {


    fn init(&mut self, heap_bottom: usize, heap_size: usize) {
        self.bottom = heap_bottom;
        self.size = heap_size;
        self.holes = unsafe { HoleList::new(heap_bottom, heap_size) };
    }

    fn new(heap_bottom: usize, heap_size: usize) -> ListHeap {
        ListHeap {
            bottom: heap_bottom,
            size: heap_size,
            holes: unsafe { HoleList::new(heap_bottom, heap_size) }
        }
    }

    fn allocate(&mut self, layout: Layout) -> 
        Result<NonNull<u8>, AllocErr> {
            self.allocate_first_fit(layout)
    }


    unsafe fn deallocate (&mut self, ptr: NonNull<u8>, layout: Layout) {
        let mut size = layout.size();
        if size < HoleList::min_size() {
            size = HoleList::min_size();
        }
        let size = align_up(size, align_of::<Hole>());
        let layout = Layout::from_size_align(size, layout.align()).unwrap();
        self.holes.deallocate(ptr, layout);
    }

}

fn allocate_first_fit(mut previous: &mut Hole, layout: Layout) -> Result<Allocation, AllocErr> {
    loop {
        let allocation: Option<Allocation> = previous
            .next
            .as_mut()
            .and_then(|current| split_hole(current.info(), layout.clone()));
        match allocation {
            Some(allocation) => {
                previous.next = previous.next.as_mut().unwrap().next.take();
                return Ok(allocation);
            }
            None if previous.next.is_some() => {
                previous = move_helper(previous).next.as_mut().unwrap();
            }
            None => {
                return Err(AllocErr);
            }
        }
    }
}

fn deallocate(mut hole: &mut Hole, addr: usize, mut size: usize) {
    loop {
        let hole_addr = if hole.size == 0 {
            0
        } else {
            hole as *mut _ as usize
        };

        let next_hole_info = hole.next.as_ref().map(|next| next.info());

        match next_hole_info {
            Some(next) if hole_addr + hole.size == addr && addr + size == next.addr => {
                hole.size += size + next.size;
                hole.next = hole.next.as_mut().unwrap().next.take();
            }
            _ if hole_addr + hole.size == addr => {
                hole.size += size;
            }

            Some(next) if addr + size == next.addr => {
                hole.next = hole.next.as_mut().unwrap().next.take();
                size += next.size;
                continue;
            }
            Some(next) if next.addr <= addr => {
                hole = move_helper(hole).next.as_mut().unwrap();
                continue;
            }
            _ => {
                let new_hole = Hole {
                    size: size,
                    next: hole.next.take()
                };

                let ptr = addr as *mut Hole;
                unsafe { ptr.write(new_hole) };
                hole.next = Some(unsafe { &mut *ptr });
            }
        }
        break;
    }

}

struct Hole {
    size: usize,
    next: Option<&'static mut Hole>,
}

impl Hole {
    fn info(&self) -> HoleInfo {
        HoleInfo {
            addr: self as *const _ as usize,
            size: self.size
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HoleInfo {
    addr: usize,
    size: usize
}

struct HoleList {
    first: Hole
}

impl HoleList {
    pub const fn empty() -> HoleList {
        HoleList {
            first: Hole {
                size: 0,
                next: None,
            }
        }
    }

    pub unsafe fn new(hole_addr: usize, hole_size: usize) -> HoleList {

        let aligned_hole_addr = align_up(hole_addr, align_of::<Hole>());
        let ptr = aligned_hole_addr as *mut Hole;

        ptr.write(Hole {
            size: hole_size.saturating_sub(aligned_hole_addr - hole_addr),
            next: None
        });
        
        HoleList {
            first: Hole {
                size: 0,
                next: Some(&mut *ptr)
                }
        }
    }

    pub fn min_size() -> usize {
        size_of::<usize>() * 2
    }

    pub fn allocate_first_fit(&mut self, layout: Layout) -> Result<NonNull<u8>, AllocErr> {

        assert!(layout.size() >= Self::min_size());

        allocate_first_fit(&mut self.first, layout).map(|allocation| {
            if let Some(padding) = allocation.front_padding {
                deallocate(&mut self.first, padding.addr, padding.size);
            }
            if let Some(padding) = allocation.back_padding {
                deallocate(&mut self.first, padding.addr, padding.size);
            }
            NonNull::new(allocation.info.addr as *mut u8).unwrap()
        })
    }

    pub fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        deallocate(&mut self.first, ptr.as_ptr() as usize, layout.size())
    }
}

fn split_hole(hole: HoleInfo, required_layout: Layout) -> Option<Allocation> {

    let required_size = required_layout.size();
    let required_align = required_layout.align();

    let (aligned_addr, front_padding) = if hole.addr == align_up(hole.addr, required_align) {
        (hole.addr, None)
    } else {
        let aligned_addr = align_up(hole.addr + HoleList::min_size(), required_align);
        ( aligned_addr, Some(HoleInfo {
            addr: hole.addr,
            size: aligned_addr - hole.addr,
        }))
    };

    let aligned_hole = {
        if aligned_addr + required_size > hole.addr + hole.size {
            return None;
        }
        HoleInfo {
            addr: aligned_addr,
            size: hole.size - (aligned_addr - hole.addr)
        }
    };

    let back_padding = if aligned_hole.size == required_size {
        None
    } else if aligned_hole.size - required_size < HoleList::min_size() {
        return None;
    } else {
        Some(HoleInfo {
            addr: aligned_hole.addr + required_size,
            size: aligned_hole.size - required_size,
        })
    };

    Some(Allocation {
        info: HoleInfo {
            addr: aligned_hole.addr, 
            size: required_size
        },
        front_padding,
        back_padding
    })
}
