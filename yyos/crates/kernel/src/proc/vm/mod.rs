use crate::{humanized_size, memory::*};
use alloc::{format, vec::Vec};
use x86_64::{
    VirtAddr,
    structures::paging::{
        mapper::{CleanUp, UnmapError},
        page::*,
        *,
    },
};
use xmas_elf::ElfFile;

pub mod heap;
pub mod stack;

use self::{heap::Heap, stack::Stack};

use super::{PageTableContext, ProcessId};

// See the documentation for the `KernelPages` type
// Ignore when you not reach this part
//
use boot::KernelPages;

type MapperRef<'a> = &'a mut OffsetPageTable<'static>;
type FrameAllocatorRef<'a> = &'a mut BootInfoFrameAllocator;

pub struct ProcessVm {
    // page table is shared by parent and child
    pub(super) page_table: PageTableContext,

    // stack is pre-process allocated
    pub(super) stack: Stack,

    // heap is allocated by brk syscall
    pub(super) heap: Heap,

    // code is hold by the first process
    // these fields will be empty for other processes
    pub(super) code: Vec<PageRangeInclusive>,
    pub(super) code_usage: u64,
}

impl ProcessVm {
    pub fn new(page_table: PageTableContext) -> Self {
        Self {
            page_table,
            stack: Stack::empty(),
            heap: Heap::empty(),
            code: Vec::new(),
            code_usage: 0,
        }
    }

    // See the documentation for the `KernelPages` type
    // Ignore when you not reach this part

    /// Initialize kernel vm
    ///
    /// NOTE: this function should only be called by the first process
    pub fn init_kernel_vm(mut self, pages: &KernelPages) -> Self {
        self.code.extend(pages.iter().copied());
        self.code_usage = self
            .code
            .iter()
            .map(|range| range.clone().count() as u64 * PAGE_SIZE)
            .sum();
        self.stack = Stack::kstack();
        self
    }

    pub fn brk(&self, addr: Option<VirtAddr>) -> Option<VirtAddr> {
        self.heap.brk(
            addr,
            &mut self.page_table.mapper(),
            &mut get_frame_alloc_for_sure(),
        )
    }

    pub fn load_elf(&mut self, elf: &ElfFile) {
        let Self {
            page_table,
            code,
            code_usage,
            ..
        } = self;
        let mapper = &mut page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        Self::load_elf_code(code, code_usage, elf, mapper, alloc);
    }

    fn load_elf_code(
        code: &mut Vec<PageRangeInclusive>,
        code_usage: &mut u64,
        elf: &ElfFile,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) {
        *code = elf::load_elf(elf, *PHYSICAL_OFFSET.get().unwrap(), mapper, alloc, true)
            .expect("Failed to load ELF code");

        *code_usage = code
            .iter()
            .map(|range| range.clone().count() as u64 * PAGE_SIZE)
            .sum();
    }

    pub fn init_proc_stack(&mut self, pid: ProcessId) -> VirtAddr {
        let stack_base = stack::STACK_MAX - stack::STACK_MAX_SIZE * (u16::from(pid) as u64 + 1);
        let stack_bot = stack_base + stack::STACK_MAX_SIZE - stack::STACK_DEF_SIZE;
        let stack_top = VirtAddr::new(stack_base + stack::STACK_MAX_SIZE - 8);
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        self.stack.init_at(stack_bot, mapper, alloc, true);
        stack_top
    }

    pub fn fork(&self, stack_offset_count: u64) -> Self {
        let owned_page_table = self.page_table.fork();
        let mapper = &mut owned_page_table.mapper();

        let alloc = &mut *get_frame_alloc_for_sure();

        Self {
            page_table: owned_page_table,
            stack: self.stack.fork(mapper, alloc, stack_offset_count),
            heap: self.heap.fork(),

            code: self.code.clone(),
            code_usage: self.code_usage,
        }
    }

    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> bool {
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        self.stack.handle_page_fault(addr, mapper, alloc)
    }

    pub(super) fn memory_usage(&self) -> u64 {
        self.stack.memory_usage() + self.heap.memory_usage() + self.code_usage
    }

    pub(super) fn clean_up(&mut self) -> Result<(), UnmapError> {
        let mapper = &mut self.page_table.mapper();
        let dealloc = &mut *get_frame_alloc_for_sure();

        self.stack.clean_up(mapper, dealloc)?;

        if self.page_table.using_count() == 1 {
            // free heap
            self.heap.clean_up(mapper, dealloc)?;

            // free code
            for page_range in self.code.iter() {
                elf::unmap_range(*page_range, mapper, dealloc, true)?;
            }

            unsafe {
                // free P1-P3
                mapper.clean_up(dealloc);
            }
        }

        // NOTE: maybe print how many frames are recycled
        //       **you may need to add some functions to `BootInfoFrameAllocator`**

        Ok(())
    }
}

impl Drop for ProcessVm {
    fn drop(&mut self) {
        if let Err(err) = self.clean_up() {
            error!("Failed to clean up process virtual memory: {:?}", err);
        }
    }
}

impl core::fmt::Debug for ProcessVm {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let (size, unit) = humanized_size(self.memory_usage());

        f.debug_struct("ProcessVm")
            .field("stack", &self.stack)
            .field("heap", &self.heap)
            .field("memory_usage", &format!("{} {}", size, unit))
            .field("page_table", &self.page_table)
            .finish()
    }
}
