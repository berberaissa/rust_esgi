/// Memory management: page-table mapper and physical-frame allocator.
///
/// `init()` returns a `MappedPageTable` that can map virtual pages to
/// physical frames. `BootInfoFrameAllocator` wraps the bootloader's
/// memory map and hands out usable physical frames one at a time.
use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB,
    },
};

/// Initialises the page-table mapper.
///
/// # Safety
///
/// `physical_memory_offset` must be the exact offset at which the
/// bootloader has identity-mapped all physical memory.
/// Must be called only once.
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Returns a mutable reference to the active level-4 page table.
///
/// # Safety
///
/// Same contract as `init()`.
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;
    let (level_4_table_frame, _) = Cr3::read();
    let phys                     = level_4_table_frame.start_address();
    let virt                     = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    &mut *page_table_ptr
}

// ── Physical-frame allocator ──────────────────────────────────────────────────

/// A frame allocator that uses the bootloader's memory map.
///
/// It iterates through all `USABLE` memory regions and hands out frames
/// sequentially. Freed frames are not reclaimed — this is intentional
/// for simplicity during early boot.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next:       usize,
}

impl BootInfoFrameAllocator {
    /// Creates a new allocator from the bootloader's memory map.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `memory_map` is valid and that all
    /// frames marked `USABLE` are genuinely free.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator { memory_map, next: 0 }
    }

    /// Returns an iterator over all usable physical frames.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        self.memory_map
            .iter()
            .filter(|r| r.region_type == MemoryRegionType::Usable)
            .map(|r| r.range.start_addr()..r.range.end_addr())
            .flat_map(|r| r.step_by(4096))
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
