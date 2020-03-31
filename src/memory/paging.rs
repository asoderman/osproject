use x86_64::{
    structures::paging::{PageTable, OffsetPageTable, 
        Size4KiB, Mapper, mapper::MapperFlush, mapper::MapToError, FrameAllocator, page::Page, page_table::PageTableFlags},
    VirtAddr,
    PhysAddr
};

use super::BootInfoFrameAllocator;

use bootloader::bootinfo::{MemoryRegionType, MemoryMap};

pub struct MemoryManager {
    pub frame_allocator: BootInfoFrameAllocator,
}

impl MemoryManager {
    pub fn new(allocator: BootInfoFrameAllocator) -> MemoryManager {
        MemoryManager {
            frame_allocator: allocator,
        }
    }

    pub fn map(&mut self, addr: VirtAddr, mapper: &mut OffsetPageTable) 
        -> Result<MapperFlush<Size4KiB>, MapToError<Size4KiB>> {
            let page = Page::from_start_address(addr).unwrap();
            if let Some(frame) = self.frame_allocator.allocate_frame() {
                mapper.map_to(page, frame, PageTableFlags::PRESENT, &mut self.frame_allocator)
            } else {
                Err(MapToError::FrameAllocationFailed)
            }
    }

    pub fn request_address_space(&mut self) {

    }

    pub fn request_address_space_at(&mut self, addr: VirtAddr) {

    }

    pub fn relinquish_address_space(&mut self, addr: VirtAddr) {

    }
}

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
