use alloc::format;

use x86_64::{
    VirtAddr,
    structures::paging::{page::*, *},
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

    pub fn init_kernel_vm(mut self) -> Self {
        // TODO: record kernel code usage
        self.stack = Stack::kstack();
        self
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

        self.stack.init_at(init_bot, mapper, alloc);

        stack_top_addr
    }

    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> bool {
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        self.stack.handle_page_fault(addr, mapper, alloc)
    }

    pub(super) fn memory_usage(&self) -> u64 {
        self.stack.memory_usage()
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
