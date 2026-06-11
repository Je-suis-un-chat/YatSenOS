use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, mapper::{MapToError, UnmapError}, page::*},
};

use super::{FrameAllocatorRef, MapperRef};
use crate::memory::physical_to_virtual;

// 0xffff_ff00_0000_0000 is the kernel's address space
pub const STACK_MAX: u64 = 0x4000_0000_0000;
pub const STACK_MAX_PAGES: u64 = 0x100000;
pub const STACK_MAX_SIZE: u64 = STACK_MAX_PAGES * crate::memory::PAGE_SIZE;
pub const STACK_START_MASK: u64 = !(STACK_MAX_SIZE - 1);
// [bot..0x2000_0000_0000..top..0x3fff_ffff_ffff]
// init stack
pub const STACK_DEF_BOT: u64 = STACK_MAX - STACK_MAX_SIZE;
pub const STACK_DEF_PAGE: u64 = 1;
pub const STACK_DEF_SIZE: u64 = STACK_DEF_PAGE * crate::memory::PAGE_SIZE;

pub const STACK_INIT_BOT: u64 = STACK_MAX - STACK_DEF_SIZE;
pub const STACK_INIT_TOP: u64 = STACK_MAX - 8;

const STACK_INIT_TOP_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(STACK_INIT_TOP));

// [bot..0xffffff0100000000..top..0xffffff01ffffffff]
// kernel stack
pub const KSTACK_MAX: u64 = 0xffff_ff02_0000_0000;
pub const KSTACK_DEF_BOT: u64 = KSTACK_MAX - STACK_MAX_SIZE;
pub const KSTACK_DEF_PAGE: u64 = 8; // Initially mapped kernel stack pages.
pub const KSTACK_DEF_SIZE: u64 = KSTACK_DEF_PAGE * crate::memory::PAGE_SIZE;

pub const KSTACK_INIT_BOT: u64 = KSTACK_MAX - KSTACK_DEF_SIZE;
pub const KSTACK_INIT_TOP: u64 = KSTACK_MAX - 8;

const KSTACK_INIT_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(KSTACK_INIT_BOT));
const KSTACK_END_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(KSTACK_MAX));

#[derive(Clone, Copy)]
pub struct Stack {
    pub(super) range: PageRange<Size4KiB>,
    usage: u64,
    user_access: bool,
}

impl Stack {
    pub fn init_at(
        &mut self,
        stack_bot:u64,
        mapper: MapperRef,
        alloc:FrameAllocatorRef,
        user_access: bool,
    ){
        debug_assert!(self.usage == 0, "Stack is not empty.");
        let flags = if user_access {
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE
        } else {
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE
        };
        self.range = elf::map_range(stack_bot, STACK_DEF_PAGE, mapper, alloc, flags).unwrap();
        self.usage = STACK_DEF_PAGE;
        self.user_access = user_access;
    }
    pub fn new(top: Page, size: u64) -> Self {
        Self {
            range: Page::range(top - size + 1, top + 1),
            usage: size,
            user_access: true,
        }
    }

    pub const fn empty() -> Self {
        Self {
            range: Page::range(STACK_INIT_TOP_PAGE, STACK_INIT_TOP_PAGE),
            usage: 0,
            user_access: true,
        }
    }

    pub const fn kstack() -> Self {
        Self {
            range: Page::range(KSTACK_INIT_PAGE, KSTACK_END_PAGE),
            usage: KSTACK_DEF_PAGE,
            user_access: false,
        }
    }

    pub fn init(&mut self, mapper: MapperRef, alloc: FrameAllocatorRef) {
        debug_assert!(self.usage == 0, "Stack is not empty.");
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        self.range = elf::map_range(STACK_INIT_BOT, STACK_DEF_PAGE, mapper, alloc, flags).unwrap();
        self.usage = STACK_DEF_PAGE;
        self.user_access = true;
    }

    pub fn handle_page_fault(
        &mut self,
        addr: VirtAddr,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> bool {
        if !self.is_on_stack(addr) {
            return false;
        }

        if let Err(m) = self.grow_stack(addr, mapper, alloc) {
            error!("Grow stack failed: {:?}", m);
            return false;
        }

        true
    }

    fn is_on_stack(&self, addr: VirtAddr) -> bool {
        let addr = addr.as_u64();
        let cur_stack_bot = self.range.start.start_address().as_u64();
        trace!("Current stack bot: {:#x}", cur_stack_bot);
        trace!("Address to access: {:#x}", addr);
        // Is it within the STACK_MAX_SIZE capacity?
        let max_stack_top = (cur_stack_bot & STACK_START_MASK) + STACK_MAX_SIZE;
        addr >= (cur_stack_bot & STACK_START_MASK) && addr < max_stack_top
    }

    fn grow_stack(
        &mut self,
        addr: VirtAddr,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> Result<(), MapToError<Size4KiB>> {
        debug_assert!(self.is_on_stack(addr), "Address is not on stack.");

        let fault_page = Page::containing_address(addr);

        let current_bot = self.range.start;

        if fault_page >= current_bot{
            return Ok(());
        }
        let new_pages_count = current_bot - fault_page;
        let new_usage = self.usage + new_pages_count;

        if new_usage > STACK_MAX_PAGES{
            error!("Stack overflow: requested {} pages, max is {}",
                      new_usage,STACK_MAX_PAGES);
            return Err(MapToError::FrameAllocationFailed);
        }
        
        let mut flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        if self.user_access {
            flags |= PageTableFlags::USER_ACCESSIBLE;
        }
        
        for page in Page::range(fault_page,current_bot){
            let frame = alloc.allocate_frame().ok_or(MapToError::FrameAllocationFailed)?;

            unsafe {
                mapper.map_to(page, frame, flags, alloc)?.flush();
            }
        }

        self.range = Page::range(fault_page, self.range.end);
        self.usage = new_usage;

        Ok(())
    }

    pub fn memory_usage(&self) -> u64 {
        self.usage * crate::memory::PAGE_SIZE
    }

    pub(super) fn clean_up(
        &mut self,
        mapper: MapperRef,
        dealloc: FrameAllocatorRef,
    ) -> Result<(), UnmapError> {
        if self.usage == 0 {
            return Ok(());
        }

        let range = Page::range_inclusive(self.range.start, self.range.end - 1);
        elf::unmap_range(range, mapper, dealloc, true)?;
        *self = Self::empty();
        Ok(())
    }

    pub fn fork(&self, mapper: MapperRef, alloc: FrameAllocatorRef, stack_offset_count: u64,)
    -> Self{
        let stack_offset = stack_offset_count * STACK_MAX_SIZE;
        let offset_pages = stack_offset / crate::memory::PAGE_SIZE;

        let new_start = self.range.start - offset_pages;
        let new_end = self.range.end - offset_pages;
        let new_range = Page::range(new_start, new_end);
        let old_stack_start = self.range.start.start_address().as_u64();
        let old_stack_end = self.range.end.start_address().as_u64();

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        
        for (source_page, target_page) in self.range.clone().zip(new_range.clone()){
            let frame = alloc
            .allocate_frame()
            .expect("Cannot allocate frame for child stack");

        unsafe {
            mapper
                .map_to(target_page, frame, flags, alloc)
                .expect("Cannot map child stack")
                .flush();

            let source = source_page.start_address().as_ptr::<u8>();
            let target = physical_to_virtual(
                frame.start_address().as_u64(),
            ) as *mut u8;

            core::ptr::copy_nonoverlapping(
                source,
                target,
                crate::memory::PAGE_SIZE as usize,
            );

            // Relocate saved frame pointers and other stack-relative pointers.
            for index in 0..crate::memory::PAGE_SIZE as usize / size_of::<usize>() {
                let slot = target.cast::<usize>().add(index);
                let value = slot.read();

                if value >= old_stack_start as usize && value < old_stack_end as usize {
                    slot.write(value - stack_offset as usize);
                }
            }
        }
        }

        Self {
            range: new_range,
            usage: self.usage,
            user_access: self.user_access,
        }
    }
}

impl core::fmt::Debug for Stack {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Stack")
            .field(
                "top",
                &format_args!("{:#x}", self.range.end.start_address().as_u64()),
            )
            .field(
                "bot",
                &format_args!("{:#x}", self.range.start.start_address().as_u64()),
            )
            .finish()
    }
}
