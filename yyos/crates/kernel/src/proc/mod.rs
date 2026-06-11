pub mod context;
pub mod data;
pub mod manager;
pub mod paging;
pub mod pid;
pub mod process;
pub mod processor;
pub mod sync;
pub mod vm;

use alloc::{
    format,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use xmas_elf::ElfFile;

pub use context::ProcessContext;
pub use data::ProcessData;
use manager::*;
pub use paging::PageTableContext;
pub use pid::ProcessId;
use process::*;
use vm::ProcessVm;
use x86_64::{VirtAddr, structures::idt::PageFaultErrorCode};

use crate::memory::allocator::HEAP_SIZE;
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
    let proc_vm = ProcessVm::new(PageTableContext::new()).init_kernel_vm(&boot_info.kernel_pages);

    trace!("Init kernel vm: {:#?}", proc_vm);
     
    let mut proc_data = ProcessData::new();
    
     // 从 boot.conf 配置中获取的信息（硬编码或从配置解析）
    proc_data.set_env("KERNEL_STACK_ADDR", "0xFFFFFF0100000000");
    proc_data.set_env("KERNEL_STACK_SIZE", "1048576");
    proc_data.set_env("KERNEL_PATH", "\\KERNEL.ELF");
        
    // 从 BootInfo 获取的信息
    proc_data.set_env("PHYSICAL_MEM_OFFSET", format!("{:#x}", boot_info.physical_memory_offset).as_str());
    proc_data.set_env("SYSTEM_TABLE", format!("{:#x}", boot_info.system_table.as_ptr() as u64).as_str());
        
    // 从运行时获取的信息
    proc_data.set_env("KERNEL_HEAP_SIZE", format!("{}", HEAP_SIZE).as_str());

    // kernel process
    let kproc = { 
        Process::new(
            String::from("kernel"),
            None,
            Some(proc_vm),
            Some(proc_data),
        )
        };
    let app_list = boot_info.loaded_apps.clone();
    manager::init(kproc, app_list);

    info!("Process Manager Initialized.");
}

pub fn switch(context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().save_current(context);

        let current = get_process_manager().current();
        if current.read().status() == ProgramStatus::Ready{
            get_process_manager().push_ready(current.pid());
        }
        get_process_manager().switch_next(context);
    });
}
/* 
pub fn spawn_kernel_thread(entry: fn() -> !, name: String, data: Option<ProcessData>) -> ProcessId {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let entry = VirtAddr::new(entry as usize as u64);
        get_process_manager().spawn_kernel_thread(entry, name, data)
    })
}*/

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

pub fn list_app() {
    // 占位函数，后续实验需要实现：遍历并列出可用的 app
    x86_64::instructions::interrupts::without_interrupts(|| {
        let app_list = get_process_manager().app_list();
        if app_list.is_none() {
            println!("[!] No app found in list!");
            return;
        }

        let apps = app_list
            .unwrap()
            .iter()
            .map(|app| app.name.as_str())
            .collect::<Vec<&str>>()
            .join(", ");

        println!("[+] App list: {}", apps);
    });
}

pub fn spawn(name: &str) -> Option<ProcessId> {
    // 占位函数，后续实验需要实现：解析 app 的 ELF 文件，并创建用户态进程
    let app = x86_64::instructions::interrupts::without_interrupts(|| {
        let app_list = get_process_manager().app_list()?;
        app_list.iter().find(|&app| app.name.eq(name))
    })?;

    elf_spawn(name.to_string(), &app.elf)
}

pub fn elf_spawn(name: String, elf: &ElfFile) -> Option<ProcessId> {
    let pid = x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let process_name = name.to_lowercase();
        let parent = Arc::downgrade(&manager.current());
        let pid = manager.spawn(elf, name, Some(parent), None);

        debug!("Spawned process: {}#{}", process_name, pid);
        pid
    });

    Some(pid)
}

pub fn brk(addr: Option<VirtAddr>) -> Option<VirtAddr> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().current().read().brk(addr)
    })
}

pub fn read(fd: u8, buf: &mut [u8]) -> isize {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().read(fd, buf))
}

pub fn write(fd: u8, buf: &[u8]) -> isize {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().write(fd, buf))
}

pub fn exit(ret: isize, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let exiting = manager.current();

        exiting.kill(ret);

        let old_pid = exiting.pid();

        manager.wake_up_waiter(old_pid, ret);
        let next_pid = manager.switch_next(context);

        if next_pid == old_pid{
            return;
        }

        let (proc_vm, proc_data) = exiting.write().take_resources();

        drop(proc_data);
        drop(proc_vm);
    })
}

#[inline]
pub fn still_alive(pid: ProcessId) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // check if the process is still alive
        let proc = get_process_manager().get_proc(&pid);
        match proc{
            Some(p) => p.read().status() != ProgramStatus::Dead,
            None => false,
        }
    })
}

pub fn fork(context:&mut ProcessContext){
    x86_64::instructions::interrupts::without_interrupts(||{
        let manager = get_process_manager();
        let parent_pid = manager.current().pid();

        manager.save_current(context);

        let child_pid = manager.fork();

        manager.push_ready(child_pid);
        manager.push_ready(parent_pid);

        manager.switch_next(context);
    })
}


pub fn wait_pid(pid: ProcessId, context:&mut ProcessContext){
    x86_64::instructions::interrupts::without_interrupts(||{
        let manager = get_process_manager();
        if let Some(ret) = manager.get_exit_code(pid){
            context.set_rax(ret as usize);
            return;
        }
        if manager.get_proc(&pid).is_none(){
            context.set_rax((-1isize) as usize);
            return;
        }
        manager.wait_pid(pid);
        manager.save_current(context);
        manager.current().write().block();
        manager.switch_next(context);
    })
}