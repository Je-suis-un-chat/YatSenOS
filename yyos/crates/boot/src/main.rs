#![no_std]
#![no_main]

#[macro_use]
extern crate log;
extern crate alloc;

use uefi::mem::memory_map::MemoryMap;
use uefi::{Status, entry, boot::MemoryType};
use x86_64::registers::control::*;
use yyos_boot::*;

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

    // 3. 计算物理内存最大边界以进行线性映射
    let mmap = uefi::boot::memory_map(MemoryType::LOADER_DATA).expect("Failed to get memory map");
    let max_phys_addr = mmap
        .entries()
        .map(|m| m.phys_start + m.page_count * 0x1000)
        .max()
        .unwrap_or(0x1_0000_0000)
        .max(0x1_0000_0000);

    // 4. 建立虚拟内存映射
    let mut page_table = current_page_table();
    let mut frame_allocator = UEFIFrameAllocator;

    unsafe {
        Cr0::update(|f| f.remove(Cr0Flags::WRITE_PROTECT));
    }

    elf::map_physical_memory(
        config.physical_memory_offset,
        max_phys_addr,
        &mut page_table,
        &mut frame_allocator,
    );

    elf::load_elf(
        &elf,
        config.physical_memory_offset,
        &mut page_table,
        &mut frame_allocator,
        false, // Kernel is not user accessible
    ).expect("Failed to load and map ELF segments");

    elf::map_range(
        config.kernel_stack_address,
        config.kernel_stack_size,
        &mut page_table,
        &mut frame_allocator,
        x86_64::structures::paging::PageTableFlags::PRESENT | x86_64::structures::paging::PageTableFlags::WRITABLE,
    ).expect("Failed to map kernel stack");

    unsafe {
        Cr0::update(|f| f.insert(Cr0Flags::WRITE_PROTECT));
    }

    free_elf(elf);

    let loaded_apps = if config.load_apps {
        info!("Loading apps....");
        Some(load_apps())
    } else {
        info!("Skip loading apps");
        None
    };

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
        loaded_apps,
    };

    let stacktop = config.kernel_stack_address + config.kernel_stack_size * 0x1000 - 8;
    jump_to_entry(&bootinfo, stacktop)
}


