pub mod context;
pub mod data;
pub mod manager;
pub mod paging;
pub mod pid;
pub mod process;
pub mod processor;
pub mod vm;

use alloc::{format, string::String};

pub use context::ProcessContext;
pub use data::ProcessData;
use manager::*;
pub use paging::PageTableContext;
pub use pid::ProcessId;
use process::*;
use vm::ProcessVm;
use x86_64::{VirtAddr, structures::idt::PageFaultErrorCode};

use crate::memory::{PAGE_SIZE, allocator::HEAP_SIZE};
pub const KERNEL_PID: ProcessId = ProcessId(1);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProgramStatus {
    Running,
    Ready,
    Blocked,
    Dead,
}

/// init process manager
pub fn init(boot_info: &'static boot::BootInfo) {
    let proc_vm = ProcessVm::new(PageTableContext::new()).init_kernel_vm();

    trace!("Init kernel vm: {:#?}", proc_vm);
     
    let mut proc_data = ProcessData::new();
    
     // 从 boot.conf 配置中获取的信息（硬编码或从配置解析）
    proc_data.set_env("KERNEL_STACK_ADDR", "0xFFFFFF0100000000");
    proc_data.set_env("KERNEL_STACK_SIZE", "512");
    proc_data.set_env("KERNEL_PATH", "\\KERNEL.ELF");
        
    // 从 BootInfo 获取的信息
    proc_data.set_env("PHYSICAL_MEM_OFFSET", format!("{:#x}", boot_info.physical_memory_offset).as_str());
    proc_data.set_env("SYSTEM_TABLE", format!("{:#x}", boot_info.system_table.as_ptr() as u64).as_str());
        
    // 从运行时获取的信息
    proc_data.set_env("KERNEL_HEAP_SIZE", format!("{}", HEAP_SIZE).as_str());

    // kernel process
    let kproc = { 
        /* FIXME: create kernel process */
        Process::new(
            String::from("kernel"),
            None,
            Some(proc_vm),
            Some(proc_data),
        )
        };
    manager::init(kproc);

    info!("Process Manager Initialized.");
}

pub fn switch(context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // FIXME: switch to the next process
        get_process_manager().save_current(context);

        let current = get_process_manager().current();
        if current.read().status() == ProgramStatus::Ready{
            get_process_manager().push_ready(current.pid());
        }
        get_process_manager().switch_next(context);
    });
}

pub fn spawn_kernel_thread(entry: fn() -> !, name: String, data: Option<ProcessData>) -> ProcessId {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let entry = VirtAddr::new(entry as usize as u64);
        get_process_manager().spawn_kernel_thread(entry, name, data)
    })
}

pub fn print_process_list() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().print_process_list();
    })
}

pub fn env(key: &str) -> Option<String> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let current = get_process_manager().current();
        let inner = current.read();

        // Rust 自动插入 Deref 调用：
        // inner.env(key)
        // ↓ 自动解引用 RwLockReadGuard → &ProcessInner
        // (*inner).env(key)  
        // ↓ ProcessInner 的 Deref 实现 → &ProcessData
        // ProcessData::env(&**inner, key)

        inner.env(key)
    })
}

pub fn process_exit(ret: isize) -> ! {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().kill_current(ret);
    });

    loop {
        x86_64::instructions::hlt();
    }
}

pub fn handle_page_fault(addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().handle_page_fault(addr, err_code)
    })
}
