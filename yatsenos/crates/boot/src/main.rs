#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate log;
extern crate alloc;

use alloc::{boxed::Box, vec};
use uefi::mem::memory_map::MemoryMap; 
use uefi::{Status, entry, boot::MemoryType};
use x86_64::registers::control::*;
use ysos_boot::*;

mod config;

#[entry]
fn efi_main() -> Status {
    uefi::helpers::init().expect("Failed to initialize utilities");

    log::set_max_level(log::LevelFilter::Info);
    info!("Running UEFI bootloader...");

    // 1. 加载并解析配置文件
    let mut config_file = fs::open_file("\\EFI\\BOOT\\boot.conf");
    let config_content = fs::load_file(&mut config_file);
    let config = config::Config::parse(config_content);

    info!("Config解析成功: {:#x?}", config);

    // 2. 加载内核 ELF 文件
    let mut kernel_file = fs::open_file(config.kernel_path);
    let kernel_content = fs::load_file(&mut kernel_file);
    let elf = xmas_elf::ElfFile::new(kernel_content).expect("Failed to parse kernel ELF");

    unsafe {
        set_entry(elf.header.pt2.entry_point() as usize);
    }

    // 3. 关键：计算物理内存最大边界以进行线性映射
    let mmap = uefi::boot::memory_map(MemoryType::LOADER_DATA).expect("Failed to get memory map");
    let max_phys_addr = mmap
        .entries()
        .map(|m| m.phys_start + m.page_count * 0x1000)
        .max()
        .unwrap_or(0x1_0000_0000) // 确保覆盖到 4GB 范围
        .max(0x1_0000_0000);

    // 4. 建立虚拟内存映射
    let mut page_table = current_page_table();
    let mut frame_allocator = UEFIFrameAllocator;

    unsafe {
        // 暂时关闭写保护以修改页表
        Cr0::update(|f| f.remove(Cr0Flags::WRITE_PROTECT));
    }

    // --- 步骤 4.1: 线性映射整个物理内存 ---
    // 这是修复 Page Fault 的关键：为 load_elf 提供可写的线性地址空间
    elf::map_physical_memory(
        config.physical_memory_offset,
        max_phys_addr,
        &mut page_table,
        &mut frame_allocator,
    );


    // --- 步骤 4.2: 加载内核段 ---
    elf::load_elf(
        &elf,
        config.physical_memory_offset, 
        &mut page_table, 
        &mut frame_allocator,
    ).expect("Failed to load and map ELF segments");
    
    // --- 步骤 4.3: 映射内核栈 ---
    elf::map_range(
        config.kernel_stack_address,
        config.kernel_stack_size,
        &mut page_table,
        &mut frame_allocator,
    ).expect("Failed to map kernel stack");

    unsafe {
        // 重新开启写保护
        Cr0::update(|f| f.insert(Cr0Flags::WRITE_PROTECT));
    }

    free_elf(elf);

    // 5. 准备系统表
    let ptr = uefi::table::system_table_raw().expect("Failed to get system table");
    let system_table = ptr.cast::<core::ffi::c_void>();

    // 6. 退出引导并跳转
    info!("Exiting boot services...");
    let mmap_owned = unsafe { uefi::boot::exit_boot_services(Some(MemoryType::LOADER_DATA)) };

    let bootinfo = BootInfo {
        memory_map: mmap_owned.entries().copied().collect(),
        physical_memory_offset: config.physical_memory_offset,
        system_table,
    };

    let stacktop = config.kernel_stack_address + config.kernel_stack_size * 0x1000 - 8;
    jump_to_entry(&bootinfo, stacktop);
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}