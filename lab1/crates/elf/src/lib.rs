#![no_std]

#[macro_use]
extern crate log;

use core::ptr::{copy_nonoverlapping, write_bytes};

use x86_64::{
    PhysAddr, VirtAddr, align_up,
    structures::paging::{mapper::*, page::PageRange, *},
};
use xmas_elf::{ElfFile, program};

/// Map physical memory
///
/// map [0, max_addr) to virtual space [offset, offset + max_addr)
pub fn map_physical_memory(
    offset: u64,
    max_addr: u64,
    page_table: &mut impl Mapper<Size2MiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    trace!("Mapping physical memory...");
    let start_frame = PhysFrame::containing_address(PhysAddr::new(0));
    let end_frame = PhysFrame::containing_address(PhysAddr::new(max_addr));

    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64() + offset));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            page_table
                .map_to(page, frame, flags, frame_allocator)
                .expect("Failed to map physical memory")
                .flush();
        }
    }
}

/// Map a range of memory
///
/// allocate frames and map to specified address (R/W)
pub fn map_range(
    addr: u64,
    count: u64,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<PageRange, MapToError<Size4KiB>> {
    let range_start = Page::containing_address(VirtAddr::new(addr));
    let range_end = range_start + count;

    trace!(
        "Page Range: {:?}({})",
        Page::range(range_start, range_end),
        count
    );

    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    for page in Page::range(range_start, range_end) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        unsafe {
            page_table
                .map_to(page, frame, flags, frame_allocator)?
                .flush();
        }
    }

    Ok(Page::range(range_start, range_end))
}

/// Load & Map ELF file
pub fn load_elf(
    elf: &ElfFile,
    physical_offset: u64,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    trace!("Loading ELF file...");

    for segment in elf.program_iter() {
        if segment.get_type().unwrap() != program::Type::Load {
            continue;
        }
        load_segment(elf, physical_offset, &segment, page_table, frame_allocator)?
    }

    Ok(())
}

/// Load & Map ELF segment
fn load_segment(
    elf: &ElfFile,
    physical_offset: u64,
    segment: &program::ProgramHeader,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let virt_start_addr = VirtAddr::new(segment.virtual_addr());
    let mem_size = segment.mem_size();
    let file_size = segment.file_size();
    let file_offset = segment.offset();

    // 1. 确定权限位
    let mut page_table_flags = PageTableFlags::PRESENT;
    if segment.flags().is_write() {
        page_table_flags |= PageTableFlags::WRITABLE;
    }
    if !segment.flags().is_execute() {
        page_table_flags |= PageTableFlags::NO_EXECUTE;
    }

    trace!("Mapping segment at {:?} with flags {:?}", virt_start_addr, page_table_flags);

    // 2. 计算涉及的页面范围（按 4KiB 对齐）
    let start_page = Page::containing_address(virt_start_addr);
    let end_page = Page::containing_address(virt_start_addr + mem_size - 1u64);
    let pages = Page::range_inclusive(start_page, end_page);

    let data_src = unsafe { elf.input.as_ptr().add(file_offset as usize) };

    for (idx, page) in pages.enumerate() {
        // 申请物理帧
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        // 建立映射
        unsafe {
            page_table
                .map_to(page, frame, page_table_flags, frame_allocator)?
                .flush();
        }

        // 3. 拷贝数据与清零逻辑
        // 目标地址计算：利用物理内存偏移量访问刚刚映射的物理帧
        let dest_ptr = (frame.start_address().as_u64() + physical_offset) as *mut u8;
        
        // 计算当前页在段内的偏移
        let page_offset = idx as u64 * 4096;

        unsafe {
            if page_offset < file_size {
                // 需要从文件拷贝数据的情况
                let copy_len = core::cmp::min(file_size - page_offset, 4096);
                copy_nonoverlapping(
                    data_src.add(page_offset as usize),
                    dest_ptr,
                    copy_len as usize,
                );

                // 如果该页没填满（文件数据结束但内存段没结束），清零剩余部分
                if copy_len < 4096 {
                    write_bytes(dest_ptr.add(copy_len as usize), 0, (4096 - copy_len) as usize);
                }
            } else {
                // 纯 BSS 部分，整页清零
                write_bytes(dest_ptr, 0, 4096);
            }
        }
    }

    Ok(())
}