use x86_64::{
    structures::paging::{PageTable, OffsetPageTable, 
        Size4KiB, Mapper, mapper::MapperFlush, mapper::MapToError, FrameAllocator, page::Page, page_table::PageTableFlags},
    VirtAddr,
    PhysAddr
};

use alloc::vec::Vec;
use super::BootInfoFrameAllocator;

use bootloader::bootinfo::{MemoryRegionType, MemoryMap};

unsafe fn active_level_4_table(phys_mem_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (active_level_4_table_frame, _) = Cr3::read();
    let phys = active_level_4_table_frame.start_address();
    let virt = phys_mem_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

pub unsafe fn offset_page_table(phys_mem_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(phys_mem_offset);
    OffsetPageTable::new(level_4_table, phys_mem_offset)
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub start: VirtAddr,
    pub size: usize,
}

// TODO: Move MemoryManager and MemoryRegion to memory module root
// FIXME: Support other page sizes?
pub struct MemoryManager<A: FrameAllocator<Size4KiB>> {
    pub frame_allocator: A,
    // FIXME: This most likely needs to be a hash map, unsure of what the
    // key needs to be though.
    used_memory_regions: Vec<MemoryRegion>
}

impl<A: FrameAllocator<Size4KiB>> MemoryManager<A> {
    pub fn new(allocator: A) -> MemoryManager<A> {
        MemoryManager {
            frame_allocator: allocator,
            used_memory_regions: Vec::new()
        }
    }

    pub fn get_used_regions(&self) -> &Vec<MemoryRegion> {
        &self.used_memory_regions
    }

    pub fn heap_was_init_at(&mut self, region: MemoryRegion) {
        // Heap allocation happens before the memory manager 
        // so the memory manager can use dynamic types,
        // but we should keep track of the region being used.
        self.used_memory_regions.push(region);
    }

    fn map<M: Mapper<Size4KiB>>(&mut self, addr: VirtAddr, size: usize, mapper: &mut M) 
        -> Result<(), MapToError<Size4KiB>> {
            // FIXME: Proper error handling
            // FIXME: BUG not allocating frames for single pages. 
            // Only allocating if pages are > 1 (i.e. size > 4kb)
            let start_page = Page::containing_address(addr);

            let end_page = Page::containing_address(VirtAddr::new(addr.as_u64() + size as u64));
            let page_range = Page::range(start_page, end_page);
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            for p in page_range {
                if let Some(frame) = self.frame_allocator.allocate_frame() {
                    mapper.map_to(p, frame, flags, &mut self.frame_allocator)?.flush();
                } else {
                    return Err(MapToError::FrameAllocationFailed);
                }
            }

            Ok(())
    }

    /*
    pub fn request_address_space(&mut self, mapper: &mut offsetpagetable) -> memoryregion {

    }

    */

    pub fn request_address_space_at<M: Mapper<Size4KiB>>(&mut self, addr: VirtAddr, size: usize, mapper: &mut M) -> Result<MemoryRegion, MapToError<Size4KiB>> {
        // FIXME: clone is most likely not necessary 
        // FIXME: no error handling -- panics if memory is mapped over 
        // again
        // FIXME: size has no effect 
        self.map(addr, size, mapper)?;
        let r = MemoryRegion { 
            start: addr,
            size,
        };
        self.used_memory_regions.push(r.clone());
        Ok(r)
    }

    pub fn relinquish_address_space<M: Mapper<Size4KiB>>(&mut self, addr: VirtAddr, size: usize,  mapper: &mut M) {
        // FIXME: size has no effect
        // FIXME: error handling
        // FIXME: bug not allocating frames for single pages. 
        // only allocating if pages are > 1 (i.e. size > 4kb)
        let start_page: Page<Size4KiB> = Page::containing_address(addr);
        let end_page = Page::containing_address(VirtAddr::new(addr.as_u64() + size as u64));

        let page_range = Page::range(start_page, end_page);
        for p in page_range { 
            mapper.unmap(p).expect("could not unmap page").1.flush();
        }
        self.used_memory_regions.retain(|&region| { region.start != addr });
    }
}


#[cfg(test)]
use x86_64::structures::paging::{UnusedPhysFrame, PhysFrame};

#[cfg(test)]
struct DummyAlloc {
}

#[cfg(test)]
impl DummyAlloc {
    fn new() -> DummyAlloc {
        DummyAlloc{}
    }
}

#[cfg(test)]
unsafe impl FrameAllocator<Size4KiB> for DummyAlloc {

    fn allocate_frame(&mut self) -> Option<UnusedPhysFrame> {
        let f = PhysFrame::containing_address(PhysAddr::new(0));
        unsafe { Some(UnusedPhysFrame::new(f)) }
    }
}

#[test_case]
fn test_memory_manager() {
    // TODO: test translation when page is mapped/unmapped
    use crate::dbg_println;

    let dummy_alloc = DummyAlloc::new();

    let test_addr = VirtAddr::new(0x0f00000000);
    let mut memory_manager = MemoryManager::new(dummy_alloc);
    // using the PHYS_OFFSET global only available during testing
    let mut mapper = unsafe { 
        offset_page_table(VirtAddr::new(crate::PHYS_OFFSET))
    };

    use x86_64::structures::paging::mapper::MapperAllSizes;
    memory_manager.request_address_space_at(test_addr, 5 * 1024, &mut mapper);
    // dbg_println!("Requested address space from memory manager. Page should be mapped");
    // dbg_println!("{:?} -> {:?}", test_addr, mapper.translate(test_addr));
    // dbg_println!("Memory regions: {:?}", memory_manager.get_used_regions());
    assert!(memory_manager.get_used_regions().len() == 1);
    //dbg_println!("");
    //dbg_println!("Relinquishing address space");
    memory_manager.relinquish_address_space(test_addr, 5 * 1024, &mut mapper);
    assert!(memory_manager.get_used_regions().len() == 0);
    //dbg_println!("Memory regions: {:?}", memory_manager.get_used_regions());
    //dbg_println!("");
    memory_manager.request_address_space_at(test_addr, 1024, &mut mapper);
    assert!(memory_manager.get_used_regions().len() == 1);
    //dbg_println!("{:?} -> {:?}", test_addr, mapper.translate(test_addr));
    //dbg_println!("Memory regions: {:?}", memory_manager.get_used_regions());

}
