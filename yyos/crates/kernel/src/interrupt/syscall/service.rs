use core::alloc::Layout;

use uefi::proto::device_path::messaging::MasterSlave;

use super::SyscallArgs;
use crate::proc::manager::get_process_manager;
use crate::proc::sync::SemaphoreResult;
use crate::{proc::{self, *}, utils::*};
use crate::drivers::input::try_pop_key;

pub fn spawn_process(args: &SyscallArgs) -> usize {
    // FIXME: get app name by args
    //       - core::str::from_utf8_unchecked
    //       - core::slice::from_raw_parts
    // FIXME: spawn the process by name
    // FIXME: handle spawn error, return 0 if failed
    // FIXME: return pid as usize

    0
}

pub fn sys_write(args: &SyscallArgs) -> usize {
    // FIXME: get buffer and fd by args
    let fd = args.arg0;
    let ptr = args.arg1 as *const u8;
    let len =args.arg2;

    let buf = unsafe {
        core::slice::from_raw_parts(ptr, len)
    };
    
    // FIXME: call proc::write -> isize
    // FIXME: return the result as usize
    if fd == 1 || fd == 2{
        if let Ok(s) = core::str::from_utf8(buf){
            print!("{}", s);
        }
        len as usize
    }else{
        0
    }
   
}

pub fn sys_read(args: &SyscallArgs) -> usize {
    // FIXME: just like sys_write
    let fd = args.arg0;
    let ptr = args.arg1 as *mut u8;
    let len = args.arg2;

    let buf = unsafe {
        core::slice::from_raw_parts_mut(ptr, len)
    };

    if fd == 0 {
        let mut read_bytes = 0 ;

        while read_bytes < len {
            if let Some(ch) = try_pop_key(){
                buf[read_bytes] = ch;
                read_bytes += 1;
            }else{
                break;
            }
        }

        read_bytes
    }else{
        0
    }
    
}

pub fn exit_process(args: &SyscallArgs, context: &mut ProcessContext) {
    // FIXME: exit process with retcode
}

pub fn list_process() {
    // FIXME: list all processes
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