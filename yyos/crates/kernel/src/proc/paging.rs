use alloc::{borrow::Cow::Owned, sync::Arc};
use core::ptr::copy_nonoverlapping;

use x86_64::{
    VirtAddr,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::*,
};

use crate::memory::*;

pub struct Cr3RegValue {
    pub addr: PhysFrame,
    pub flags: Cr3Flags,
    owned: bool,
}

impl Cr3RegValue {
    pub fn new(addr: PhysFrame, flags: Cr3Flags, owned: bool,) -> Self {
       Self { addr, flags, owned }
    }
}

impl Drop for Cr3RegValue {
    fn drop(&mut self) {
        if !self.owned {
            return;
        }

        trace!(
            "Releasing page table at {:?}",
            self.addr
        );

        let mut allocator = get_frame_alloc_for_sure();

        unsafe {
            // 释放用户页面以及用户页表分支
            release_user_table(
                self.addr,
                4,
                &mut allocator,
            );

            // 最后释放 L4 自身
            allocator.deallocate_frame(self.addr);
        }
    }
}

pub struct PageTableContext {
    pub reg: Arc<Cr3RegValue>,
}

impl PageTableContext {
    pub fn new() -> Self {
        let (frame, flags) = Cr3::read();
        Self {
            reg: Arc::new(Cr3RegValue::new(frame, flags, false,)),
        }
    }
    
    /// Create a new page table object based on current page table.
    pub fn clone_level_4(&self) -> Self {
        // 1. alloc new page table
        let mut frame_alloc = crate::memory::get_frame_alloc_for_sure();
        let page_table_addr = frame_alloc
            .allocate_frame()
            .expect("Cannot alloc page table for new process.");

        // 2. copy current page table to new page table
        unsafe {
            copy_nonoverlapping::<PageTable>(
                physical_to_virtual(self.reg.addr.start_address().as_u64()) as *mut PageTable,
                physical_to_virtual(page_table_addr.start_address().as_u64()) as *mut PageTable,
                1,
            );
        }

        // 3. create page table object
        Self {
            reg: Arc::new(Cr3RegValue::new(page_table_addr, Cr3Flags::empty(), true,)),
        }
    }

    /// Load the page table to Cr3 register.
    pub fn load(&self) {
        unsafe { Cr3::write(self.reg.addr, self.reg.flags) }
    }

    /// Get the page table object by Cr3 register value.
    pub fn mapper(&self) -> OffsetPageTable<'static> {
        unsafe {
            OffsetPageTable::new(
                (physical_to_virtual(self.reg.addr.start_address().as_u64()) as *mut PageTable)
                    .as_mut()
                    .unwrap(),
                VirtAddr::new_truncate(*PHYSICAL_OFFSET.get().unwrap()),
            )
        }
    }
    pub fn using_count(&self) -> usize {
        Arc::strong_count(&self.reg)
    }
    pub fn fork(&self) -> Self{
       Self { reg: Arc::clone(&self.reg), }
    }
}

impl core::fmt::Debug for PageTableContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PageTable")
            .field("addr", &self.reg.addr)
            .field("flags", &self.reg.flags)
            .finish()
    }
}

unsafe fn release_user_table(
    frame: PhysFrame,
    level: u8,
    allocator: &mut BootInfoFrameAllocator,
) {
    let table_ptr =
        physical_to_virtual(frame.start_address().as_u64())
            as *mut PageTable;

    let table = unsafe { &mut *table_ptr };

    for index in 0..512 {
        // The user heap is mapped in the kernel page table and inherited by
        // every process through the shallow L4 clone. It is globally owned.
        let shared_heap_l4 =
            (crate::memory::user::USER_HEAP_START >> 39) & 0x1ff;
        if level == 4 && index == shared_heap_l4 {
            continue;
        }

        let entry = &mut table[index];
        let flags = entry.flags();

        if !flags.contains(PageTableFlags::PRESENT)
            || !flags.contains(PageTableFlags::USER_ACCESSIBLE)
        {
            continue;
        }

        let mapped_frame = match entry.frame() {
            Ok(frame) => frame,
            Err(_) => continue,
        };

        if level == 1 {
            // L1 表项指向用户数据、栈、堆等物理页
            entry.set_unused();

            unsafe {
                allocator.deallocate_frame(mapped_frame);
            }
        } else {
            // 本实验用户映射使用 4 KiB 页面
            assert!(
                !flags.contains(PageTableFlags::HUGE_PAGE),
                "User huge pages are not supported"
            );

            unsafe {
                release_user_table(
                    mapped_frame,
                    level - 1,
                    allocator,
                );
            }

            entry.set_unused();

            // 释放下一层页表自身占用的物理帧
            unsafe {
                allocator.deallocate_frame(mapped_frame);
            }
        }
    }
}