use core::alloc::Layout;

use crate::drivers::filesystem;
use super::SyscallArgs;
use crate::proc::manager::get_process_manager;
use crate::proc::sync::SemaphoreResult;
use crate::proc::{self, *};
use crate::resource::Resource;
use storage::FileSystem;
use x86_64::VirtAddr;

pub fn sys_write(args: &SyscallArgs) -> usize {
    let fd = args.arg0 as u8;
    let buf = unsafe { core::slice::from_raw_parts(args.arg1 as *const u8, args.arg2) };
    proc::write(fd, buf) as usize
}

pub fn sys_read(args: &SyscallArgs) -> usize {
    let fd = args.arg0 as u8;
    let ptr = args.arg1 as *mut u8;
    let len = args.arg2;

    let buf = unsafe {
        core::slice::from_raw_parts_mut(ptr, len)
    };

    proc::read(fd, buf) as usize
}

pub fn sys_brk(args: &SyscallArgs) -> usize {
    let new_end = if args.arg0 == 0 {
        None
    } else {
        match VirtAddr::try_new(args.arg0 as u64) {
            Ok(addr) => Some(addr),
            Err(_) => return usize::MAX,
        }
    };

    proc::brk(new_end)
        .map(|addr| addr.as_u64() as usize)
        .unwrap_or(usize::MAX)
}

pub fn sys_allocate(args: &SyscallArgs) -> usize {
    let layout = unsafe { (args.arg0 as *const Layout).as_ref().unwrap() };

    if layout.size() == 0 {
        return 0;
    }

    let ret = crate::memory::user::USER_ALLOCATOR
        .lock()
        .allocate_first_fit(*layout);

    match ret {
        Ok(ptr) => ptr.as_ptr() as usize,
        Err(_) => 0,
    }
}

pub fn sys_deallocate(args: &SyscallArgs) {
    let layout = unsafe { (args.arg1 as *const Layout).as_ref().unwrap() };

    if args.arg0 == 0 || layout.size() == 0 {
        return;
    }

    let ptr = args.arg0 as *mut u8;

    unsafe {
        crate::memory::user::USER_ALLOCATOR
            .lock()
            .deallocate(core::ptr::NonNull::new_unchecked(ptr), *layout);
    }
}

pub fn sys_spawn(args: &SyscallArgs) -> usize{
    let ptr = args.arg0 as *const u8;
    let len = args.arg1;
    let name = unsafe {
        core::str::from_utf8_unchecked(
            core::slice::from_raw_parts(ptr, len)
        )
    };
    
    match proc::spawn(name) {
        Some(pid) => pid.0 as usize,
        None => 0,
    }
 }

pub fn sys_getpid() -> usize{
    get_process_manager().current().pid().0 as usize
} 

pub fn sys_waitpid(args: &SyscallArgs) -> isize{
    let pid = ProcessId(args.arg0 as u16);
    let proc = get_process_manager().get_proc(&pid);
    match proc {
        Some(p) => {
            let inner = p.read();
            if inner.status() == ProgramStatus::Dead{
                inner.exit_code().unwrap_or(-1)
            }else{
                -1
            }
        }
        None => -1,
    }
}

pub fn sys_exit(args: &SyscallArgs, context: &mut ProcessContext) {
    let ret = args.arg0 as isize;
    proc::exit(ret, context);
}

pub fn sys_fork(context: & mut ProcessContext){
    proc::fork(context);
}

pub fn sys_list_app(args: &SyscallArgs) -> usize {
    let ptr = args.arg0 as *mut u8;
    let len = args.arg1;
    let buf = unsafe { core::slice::from_raw_parts_mut(ptr, len) };

    let app_list = get_process_manager().app_list();
    if app_list.is_none() {
        return 0;
    }

    let apps = app_list.unwrap();
    let mut offset = 0;
    for (i, app) in apps.iter().enumerate() {
        let name_bytes = app.name.as_str().as_bytes();
        // 检查缓冲区是否有足够空间
        if offset + name_bytes.len() > len {
            return 0; // 缓冲区太小
        }
        buf[offset..offset + name_bytes.len()].copy_from_slice(name_bytes);
        offset += name_bytes.len();
        // app 名之间插入换行分隔符（最后一个不加）
        if i < apps.len() - 1 {
            if offset + 1 > len {
                return 0;
            }
            buf[offset] = b'\n';
            offset += 1;
        }
    }
    offset // 返回实际写入的字节数
}

pub fn sys_stat() -> usize {
    proc::print_process_list();
    0
}

pub fn sem_wait(key: u32, context: &mut ProcessContext){
    x86_64::instructions::interrupts::without_interrupts(||{
        let manager = get_process_manager();
        let pid = processor::get_pid();
        let ret = manager.current().write().sem_wait(key, pid);
        match ret{
            SemaphoreResult::Ok => context.set_rax(0),
            SemaphoreResult::NotExist => context.set_rax(1),
            SemaphoreResult::Block(pid) => {
                context.set_rax(0);
                manager.save_current(context);
                manager.block(pid);
                manager.switch_next(context);
            }
            _ => unreachable!(),
        }
    })
}
pub fn sem_signal(key: u32,context: &mut ProcessContext)
{
    let manager = get_process_manager();

    match manager.current().read().sem_signal(key){
        SemaphoreResult::Ok => context.set_rax(0),
        SemaphoreResult::NotExist => context.set_rax(1),
        SemaphoreResult::WakeUp(pid) => {
            if let Some(proc) = manager.get_proc(&pid){
                proc.write().set_return_value(0);
                proc.write().wake_up();
                manager.push_ready(pid);
            }
            context.set_rax(0);
        }
        _ => unreachable!(),
    }
}

pub fn new_sem(key: u32, value: usize) -> usize{
    let manager = get_process_manager();
    if manager.current().read().new_sem(key, value){
        0
    }else {
        1
    }
}

pub fn remove_sem(key: u32) -> usize
{
        let manager = get_process_manager();
        if manager.current().read().remove_sem(key){
            0
        }else {
            1
        }

}

pub fn sys_sem(args: &SyscallArgs, context: &mut ProcessContext)
{
    match args.arg0 {
        0 => context.set_rax(new_sem(args.arg1 as u32, args.arg2)),
        1 => context.set_rax(remove_sem(args.arg1 as u32)),
        2 => sem_signal(args.arg1 as u32, context),
        3 => sem_wait(args.arg1 as u32, context),
        _ => context.set_rax(usize::MAX),
    }
    
}

pub fn sys_list_dir(args: &SyscallArgs) -> usize {
    let ptr = args.arg0 as *const u8;
    let len = args.arg1;

    if ptr.is_null() {
        return 1;
    }

    let bytes = unsafe {
        core::slice::from_raw_parts(ptr, len)
    };

    let path = match core::str::from_utf8(bytes) {
        Ok(path) => path,
        Err(_) => return 1,
    };

    if crate::drivers::filesystem::ls(path) {
        0
    } else {
        1
    }
}

pub fn sys_open(args: &SyscallArgs) -> usize{
    let ptr = args.arg0 as *const u8;
    let len = args.arg1;

    if ptr.is_null(){
        return usize::MAX;
    }

    let bytes = unsafe {
        core::slice::from_raw_parts(ptr, len)
    };

    let path = match core::str::from_utf8(bytes){
        Ok(path) => path,
        Err(_) => return usize::MAX,
    };

    let Some(rootfs) = filesystem::get_rootfs() else {
        return usize::MAX;
    };

    let file = match rootfs.open_file(path){
        Ok(file) => file,
        Err(_) => return usize::MAX,
    };

    let manager = get_process_manager();
    let current = manager.current();
    let process = current.read();

    process.open(Resource::File(file)) as usize
}

pub fn sys_close(args: &SyscallArgs) -> usize{
    let fd = args.arg0 as u8;

    let manager = get_process_manager();
    let current = manager.current();
    let process = current.read();

    if process.close(fd){
        0
    }else{
        usize::MAX
    }
}