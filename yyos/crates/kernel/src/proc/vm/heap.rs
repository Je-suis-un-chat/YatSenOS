use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};

use x86_64::{
    VirtAddr, align_up,
    structures::paging::{
        FrameAllocator, FrameDeallocator, Mapper, Page, PageTableFlags, mapper::UnmapError,
    },
};

use super::{FrameAllocatorRef, MapperRef};

// user process runtime heap
// 0x100000000 bytes -> 4GiB
// from 0x0000_2000_0000_0000 to 0x0000_2000_ffff_fff8
pub const HEAP_START: u64 = 0x2000_0000_0000;
pub const HEAP_PAGES: u64 = 0x100000;
pub const HEAP_SIZE: u64 = HEAP_PAGES * crate::memory::PAGE_SIZE;
pub const HEAP_END: u64 = HEAP_START + HEAP_SIZE - 8;

/// User process runtime heap
///
/// always page aligned, the range is [base, end)
pub struct Heap {
    /// the base address of the heap
    ///
    /// immutable after initialization
    base: VirtAddr,

    /// the current end address of the heap
    ///
    /// use atomic to allow multiple threads to access the heap
    end: Arc<AtomicU64>,
}

impl Heap {
    pub fn empty() -> Self {
        Self {
            base: VirtAddr::new(HEAP_START),
            end: Arc::new(AtomicU64::new(HEAP_START)),
        }
    }

    pub fn fork(&self) -> Self {
        Self {
            base: self.base,
            end: self.end.clone(),
        }
    }

    pub fn brk(
        &self,
        new_end: Option<VirtAddr>,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> Option<VirtAddr> {
        let Some(new_end) = new_end else {
            return Some(VirtAddr::new(self.end.load(Ordering::Acquire)));
        };

        let requested = new_end.as_u64();
        let base = self.base.as_u64();
        if !(base..=HEAP_END).contains(&requested) {
            return None;
        }

        let current = self.end.load(Ordering::Acquire);
        if requested == current {
            return Some(new_end);
        }

        let current_mapped_end = align_up(current, crate::memory::PAGE_SIZE);
        let requested_mapped_end = align_up(requested, crate::memory::PAGE_SIZE);
        trace!(
            "Adjust heap break: {:#x} -> {:#x} (mapped: {:#x} -> {:#x})",
            current, requested, current_mapped_end, requested_mapped_end
        );

        if requested_mapped_end > current_mapped_end {
            let start_page = Page::containing_address(VirtAddr::new(current_mapped_end));
            let end_page = Page::containing_address(VirtAddr::new(requested_mapped_end));
            let mut mapped_end = start_page;
            let flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;

            for page in Page::range(start_page, end_page) {
                let Some(frame) = alloc.allocate_frame() else {
                    Self::rollback_growth(start_page, mapped_end, mapper, alloc);
                    return None;
                };

                match unsafe { mapper.map_to(page, frame, flags, alloc) } {
                    Ok(flusher) => {
                        flusher.flush();
                        mapped_end = page + 1;
                    }
                    Err(_) => {
                        unsafe { alloc.deallocate_frame(frame) };
                        Self::rollback_growth(start_page, mapped_end, mapper, alloc);
                        return None;
                    }
                }
            }
        } else if requested_mapped_end < current_mapped_end {
            let start_page = Page::containing_address(VirtAddr::new(requested_mapped_end));
            let end_page = Page::containing_address(VirtAddr::new(current_mapped_end));
            if start_page < end_page {
                elf::unmap_range(
                    Page::range_inclusive(start_page, end_page - 1),
                    mapper,
                    alloc,
                    true,
                )
                .ok()?;
            }
        }

        self.end.store(requested, Ordering::Release);
        Some(new_end)
    }

    fn rollback_growth(start: Page, end: Page, mapper: MapperRef, dealloc: FrameAllocatorRef) {
        if start < end {
            let _ = elf::unmap_range(Page::range_inclusive(start, end - 1), mapper, dealloc, true);
        }
    }

    pub(super) fn clean_up(
        &self,
        mapper: MapperRef,
        dealloc: FrameAllocatorRef,
    ) -> Result<(), UnmapError> {
        if self.memory_usage() == 0 {
            return Ok(());
        }

        let base = self.base.as_u64();
        let old_end = self.end.swap(base, Ordering::AcqRel);
        let mapped_end = align_up(old_end, crate::memory::PAGE_SIZE);

        if mapped_end > base {
            let start_page = Page::containing_address(self.base);
            let end_page = Page::containing_address(VirtAddr::new(mapped_end));
            elf::unmap_range(
                Page::range_inclusive(start_page, end_page - 1),
                mapper,
                dealloc,
                true,
            )?;
        }

        Ok(())
    }

    pub fn memory_usage(&self) -> u64 {
        self.end.load(Ordering::Relaxed) - self.base.as_u64()
    }
}

impl core::fmt::Debug for Heap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Heap")
            .field("base", &format_args!("{:#x}", self.base.as_u64()))
            .field(
                "end",
                &format_args!("{:#x}", self.end.load(Ordering::Relaxed)),
            )
            .finish()
    }
}