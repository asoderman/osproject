use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector};

use lazy_static::lazy_static;

use crate::TSS;

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector
}

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector =  gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, Selectors { code_selector, tss_selector })
    };
}

pub fn init() {
    use x86_64::instructions::segmentation::set_cs;
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        set_cs(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}

pub fn set_tss(stack: usize) {
    use x86_64::VirtAddr;
    let mut tss = *TSS;
    tss.privilege_stack_table[0] = VirtAddr::new(stack as u64);
}
