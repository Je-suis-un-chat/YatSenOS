use alloc::format;
use xmas_elf::ElfFile;
use x86_64::{
    VirtAddr,
    structures::{
        idt::PageFaultErrorCode,
        paging::{mapper::MapToError, page::*, *},
    },
};

use crate::{humanized_size, memory::*};

pub mod stack;

use self::stack::*;
use super::{PageTableContext, ProcessId};

type MapperRef<'a> = &'a mut OffsetPageTable<'static>;
type FrameAllocatorRef<'a> = &'a mut BootInfoFrameAllocator;

//虚拟内存：页表+栈
pub struct ProcessVm {
    // page table is shared by parent and child
    pub(super) page_table: PageTableContext,

    // stack is pre-process allocated
    pub(super) stack: Stack,
}

impl ProcessVm {
    pub fn new(page_table: PageTableContext) -> Self {
        Self {
            page_table,
            stack: Stack::empty(),
        }
    }

    pub fn from_parts(page_table: PageTableContext, stack: Stack) -> Self {
        Self { page_table, stack }
    }

    pub fn stack_top(&self) -> VirtAddr {
        self.stack.range.end.start_address()
    }

    pub fn init_kernel_vm(mut self) -> Self {
        // TODO: record kernel code usage
        self.stack = Stack::kstack();
        self
    }

    pub fn load_elf(
        &mut self,
        elf: &ElfFile,
        user_access: bool,
    ) -> Result<(), MapToError<Size4KiB>>{
    let physical_offset = *crate::memory::PHYSICAL_OFFSET.get().unwrap();
    let mapper = &mut self.page_table.mapper();
    let frame_alloc = &mut *get_frame_alloc_for_sure();

    elf::load_elf(elf, physical_offset, mapper, frame_alloc, user_access)
    }
   

    pub fn init_proc_stack(&mut self, pid: ProcessId) -> VirtAddr {
        // FIXME: calculate the stack for pid
        let pid_value = u16::from(pid) as u64;
        let stack_base = STACK_MAX-STACK_MAX_SIZE*(pid_value+1);

        // FIXME: calculate the stack for pid
        let init_bot = stack_base + STACK_MAX_SIZE - STACK_DEF_SIZE;
        let stack_top_addr = VirtAddr::new(stack_base + STACK_MAX_SIZE - 8);

        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        // User process stacks must be USER_ACCESSIBLE (Ring 3)
        self.stack.init_at(init_bot, mapper, alloc, true);

        stack_top_addr
    }

    pub fn handle_page_fault(&mut self, addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
        // 先尝试栈增长处理
        {
            let mapper = &mut self.page_table.mapper();
            let alloc = &mut *get_frame_alloc_for_sure();
            if self.stack.handle_page_fault(addr, mapper, alloc) {
                return true;
            }
        }

        // 非栈地址缺页：对于用户空间非保护违例，尝试按需映射
        let addr_u64 = addr.as_u64();

        if addr_u64 >= 0xffff_8000_0000_0000 {
            return false;
        }
        if err_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION) {
            warn!(
                "Protection violation at {:#x} for process, cannot handle",
                addr_u64
            );
            return false;
        }

        let page = Page::containing_address(addr);
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();
        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE;

        match alloc.allocate_frame() {
            Some(frame) => {
                unsafe {
                    let result = mapper.map_to(page, frame, flags, alloc);
                    match result {
                        Ok(flusher) => {
                            // 清零新分配的帧
                            let dest = (frame.start_address().as_u64()
                                + *crate::memory::PHYSICAL_OFFSET.get().unwrap())
                                as *mut u8;
                            core::ptr::write_bytes(dest, 0, crate::memory::PAGE_SIZE as usize);
                            flusher.flush();
                            true
                        }
                        Err(e) => {
                            error!("Demand paging map_to failed: {:?}", e);
                            false
                        }
                    }
                }
            }
            None => {
                error!(
                    "Demand paging: frame allocation failed for {:#x}",
                    addr_u64
                );
                false
            }
        }
    }

    pub(super) fn memory_usage(&self) -> u64 {
        self.stack.memory_usage()
    }

    pub fn fork(&self, stack_offset_count: u64) -> Self{
        let page_table = self.page_table.fork();

        let mut mapper = page_table.mapper();
        let mut alloc = get_frame_alloc_for_sure();

        let stack = self.stack.fork(&mut mapper, &mut alloc, stack_offset_count,);

        Self { page_table, stack }
    }
}

impl core::fmt::Debug for ProcessVm {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let (size, unit) = humanized_size(self.memory_usage());

        f.debug_struct("ProcessVm")
            .field("stack", &self.stack)
            .field("memory_usage", &format!("{} {}", size, unit))
            .field("page_table", &self.page_table)
            .finish()
    }
}
