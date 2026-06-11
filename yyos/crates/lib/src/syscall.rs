//use std::random::random;

use syscall_def::Syscall;

use crate::syscall;

#[inline(always)]
pub fn sys_write(fd: u8, buf: &[u8]) -> Option<usize> {
    let ret = syscall!(
        Syscall::Write,
        fd as u64,
        buf.as_ptr() as u64,
        buf.len() as u64
    ) as isize;
    if ret.is_negative() {
        None
    } else {
        Some(ret as usize)
    }
}

#[inline(always)]
pub fn sys_read(fd: u8, buf: &mut [u8]) -> Option<usize> {
    let ret = syscall!(
        Syscall::Read,
        fd as u64,
        buf.as_ptr() as u64,
        buf.len() as u64
    ) as isize;
    if ret.is_negative() {
        None
    } else {
        Some(ret as usize)
    }
}

#[inline(always)]
pub fn sys_brk(addr: Option<usize>) -> Option<usize> {
    match syscall!(Syscall::Brk, addr.unwrap_or(0)) {
        usize::MAX => None,
        ret => Some(ret),
    }
}

#[inline(always)]
pub fn sys_wait_pid(pid: u16) -> isize {
    loop{
        let ret = syscall!(Syscall::WaitPid, pid as u64) as isize;
        if ret != -1{
            return ret;
        }
    }
    
}

#[inline(always)]
pub fn sys_list_app(buf: &mut [u8]) -> usize {
     syscall!(Syscall::ListApp, buf.as_ptr() as u64, buf.len() as u64) as usize
}

#[inline(always)]
pub fn sys_stat() {
    syscall!(Syscall::Stat);
}

#[inline(always)]
pub fn sys_allocate(layout: &core::alloc::Layout) -> *mut u8 {
    syscall!(Syscall::Allocate, layout as *const _) as *mut u8
}

#[inline(always)]
pub fn sys_deallocate(ptr: *mut u8, layout: &core::alloc::Layout) -> usize {
    syscall!(Syscall::Deallocate, ptr, layout as *const _)
}

#[inline(always)]
pub fn sys_fork() -> u16 {
    syscall!(Syscall::Fork) as u16
}

#[inline(always)]
pub fn sys_spawn(path: &str) -> u16 {
    syscall!(Syscall::Spawn, path.as_ptr() as u64, path.len() as u64) as u16
}

#[inline(always)]
pub fn sys_get_pid() -> u16 {
    syscall!(Syscall::GetPid) as u16
}

#[inline(always)]
pub fn sys_exit(code: isize) -> ! {
    syscall!(Syscall::Exit, code as u64);
    unreachable!("This process should be terminated by now.")
}

#[inline(always)]
pub fn sys_new_sem(key: u32, value: usize) -> bool {
    syscall!(Syscall::Sem, 0, key as usize, value) == 0
}
pub fn sys_remove_sem(key: u32) -> bool{
    syscall!(Syscall::Sem, 1, key as usize) == 0
}
pub fn sys_sem_signal(key: u32) -> bool
{
    syscall!(Syscall::Sem, 2, key as usize) == 0
}
pub fn sys_sem_wait(key: u32) -> bool
{
    syscall!(Syscall::Sem, 3, key as usize) == 0
}
#[inline(always)]
pub fn sys_list_dir(path: &str) -> bool {
    syscall!(
        Syscall::ListDir,
        path.as_ptr() as u64,
        path.len() as u64
    ) == 0
}

#[inline(always)]
pub fn sys_open(path: &str) -> Option<u8>{
    let ret = syscall!(
        Syscall::Open,
        path.as_ptr() as u64, 
        path.len() as u64
    ) as isize;

    if ret < 0{
        None
    }else {
        Some(ret as u8)
    }
}

#[inline(always)]
pub fn sys_close(fd: u8) -> bool{
    let ret = syscall!(Syscall::Close, fd as u64) as isize;
    ret == 0
}