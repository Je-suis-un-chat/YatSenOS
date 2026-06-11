use super::*;
use crate::memory::{
    PAGE_SIZE,
    allocator::{ALLOCATOR, HEAP_SIZE},
    get_frame_alloc_for_sure,
};
use crate::utils::macros::*;
use alloc::{
    collections::*,
    format,
    string::String,
    sync::{Arc, Weak},
};
use hashbrown::HashMap;
use spin::{Mutex, RwLock};
use x86_64::VirtAddr;
use xmas_elf::ElfFile;

pub static PROCESS_MANAGER: spin::Once<ProcessManager> = spin::Once::new();

pub fn init(init: Arc<Process>, app_list: Option<boot::AppList>) {
    init.write().resume();
    processor::set_pid(init.pid());
    PROCESS_MANAGER.call_once(|| ProcessManager::new(init, app_list));
}

pub fn get_process_manager() -> &'static ProcessManager {
    PROCESS_MANAGER
        .get()
        .expect("Process Manager has not been initialized")
}

pub struct ProcessManager {
    processes: RwLock<HashMap<ProcessId, Arc<Process>, ahash::RandomState>>,
    ready_queue: Mutex<VecDeque<ProcessId>>,
    app_list: Option<boot::AppList>,
    wait_queue: Mutex<HashMap<ProcessId, BTreeSet<ProcessId>, ahash::RandomState>>,
}

impl ProcessManager {
    //这个传入的进程会作为内核进程加入进程列表
    pub fn new(init: Arc<Process>, app_list:Option<boot::AppList>) -> Self {
        let mut processes = HashMap::default();
        let ready_queue = VecDeque::new();
        let wait_queue: HashMap<ProcessId, BTreeSet<ProcessId>, ahash::RandomState> =
            HashMap::default();
        let pid = init.pid();

        trace!("Init {:#?}", init);

        processes.insert(pid, init);
        Self {
            processes: RwLock::new(processes),
            ready_queue: Mutex::new(ready_queue),
            app_list,
            wait_queue: Mutex::new(wait_queue),
        }
    }

    #[inline]
    pub fn push_ready(&self, pid: ProcessId) {
        self.ready_queue.lock().push_back(pid);
    }

    #[inline]
    pub fn add_proc(&self, pid: ProcessId, proc: Arc<Process>) {
        self.processes.write().insert(pid, proc);
    }

    pub fn fork(&self) -> ProcessId{
       let parent = self.current();
       let child = parent.fork();
       let child_pid = child.pid();

       self.add_proc(child_pid, child);

       trace!("Ready queue: {:?}", self.ready_queue.lock());
       
       child_pid
    }

    #[inline]
    pub fn get_proc(&self, pid: &ProcessId) -> Option<Arc<Process>> {
        self.processes.read().get(pid).cloned()
    }

    pub fn current(&self) -> Arc<Process> {
        self.get_proc(&processor::get_pid())
            .expect("No current process")
    }

    pub fn save_current(&self, context: &ProcessContext) {
        let cur = self.current();
        cur.write().tick();
        cur.write().save(context);
        cur.write().pause();
    }

    pub fn switch_next(&self, context: &mut ProcessContext) -> ProcessId {
    // 1. 从就绪队列获取下一个进程
    let next_pid = loop {
        let pid = self.ready_queue.lock().pop_front();
        
        if let Some(pid) = pid {
            let proc = self.get_proc(&pid);
            
            // 2. 检查进程是否存在且就绪
            if let Some(proc) = proc {
                if proc.read().is_ready() {
                    break pid;
                }
            }
            // 如果进程不就绪，继续循环获取下一个
        } else {
            // 就绪队列空，返回当前进程 PID（无切换）
            return processor::get_pid();
        }
    };

    // 3. 获取下一个进程
    let next_proc = self.get_proc(&next_pid).unwrap();

    // 4. 恢复下一个进程的上下文
    next_proc.write().restore(context);

    // 5. 设置下一个进程为 Running 状态
    next_proc.write().resume();

    // 6. 更新处理器的当前 PID
    processor::set_pid(next_pid);

    // 7. 加载进程的页表
    next_proc.read().vm().page_table.load();

    // 8. 返回新进程的 PID
    next_pid
    }

/*  pub fn spawn_kernel_thread(
        &self,
        entry: VirtAddr,
        name: String,
        proc_data: Option<ProcessData>,
    ) -> ProcessId {
        let kproc = self.get_proc(&KERNEL_PID).unwrap();
        let page_table = kproc.read().clone_page_table();
        let proc_vm = Some(ProcessVm::new(page_table));
        let proc = Process::new(name, Some(Arc::downgrade(&kproc)), proc_vm, proc_data);

        // alloc stack for the new process base on pid
        let stack_top = proc.alloc_init_stack();
        let pid = proc.pid();
        proc.write().init_stack_frame(entry, stack_top);
        self.add_proc(pid, proc);
        self.push_ready(pid);
        pid
    } */

    pub fn kill_current(&self, ret: isize) {
        self.kill(processor::get_pid(), ret);
    }

    pub fn handle_page_fault(&self, addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
        // 1. 检查保留位违规 - 硬件错误或严重问题
        if err_code.contains(PageFaultErrorCode::MALFORMED_TABLE) {
            return false;
        }
         // 2. 检查地址是否为空指针或接近空指针
        if addr.as_u64() < 0x1000{
            return false;
        }
        // 3. 检查地址是否在有效的用户空间范围内
        if !is_canonical(addr.as_u64() as usize){
            return false;
        }

        // 7. 检查是否在保护违规情况下访问内核空间
        let user_mode = err_code.contains(PageFaultErrorCode::USER_MODE);
        let protection = err_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION);
        
        // 用户态尝试访问内核空间
        if user_mode && addr.as_u64() >= 0xffff_8000_0000_0000 && protection {
            return false; // 非法访问内核空间 - 非预期
        }

        let current = self.current();
        if current.pid() == KERNEL_PID {
            info!("Page fault on kernel at {:#x}", addr);
        }

        if current.write().handle_page_fault(addr, err_code) {
        return true; // 成功处理 - 预期异常（如栈增长）
        }
    
        
        
        false 
    }

    pub fn kill(&self, pid: ProcessId, ret: isize) {
        let proc = self.get_proc(&pid);

        if proc.is_none() {
            warn!("Process #{} not found.", pid);
            return;
        }

        let proc = proc.unwrap();

        if proc.read().status() == ProgramStatus::Dead {
            warn!("Process #{} is already dead.", pid);
            return;
        }

        trace!("Kill {:#?}", &proc);

        proc.kill(ret);
    }

    pub fn print_process_list(&self) {
        let mut output =
            String::from("  PID | PPID | Process Name |  Ticks  |   Memory    | Status\n");

        self.processes
            .read()
            .values()
            .filter(|p| p.read().status() != ProgramStatus::Dead)
            .for_each(|p| output += format!("{}\n", p).as_str());

        let frame_alloc = get_frame_alloc_for_sure();
        let frames_used = frame_alloc.frames_used();
        let frames_recycled = frame_alloc.frames_recycled();
        let frames_total = frame_alloc.frames_total();
        let memory_used = frames_used.saturating_sub(frames_recycled) * PAGE_SIZE as usize;
        let memory_total = frames_total * PAGE_SIZE as usize;
        output += &format_usage("Memory", memory_used, memory_total);
        output += format!(
            "Frames : {} used, {} recycled, {} total\n",
            frames_used, frames_recycled, frames_total
        )
        .as_str();
        drop(frame_alloc);

        // print memory usage of kernel heap
        let heap_used = ALLOCATOR.lock().used();
        let heap_free = ALLOCATOR.lock().free();
        let (used_size, used_unit) = crate::humanized_size(heap_used as u64);
        let (free_size, free_unit) = crate::humanized_size(heap_free as u64);
        let (total_size, total_unit) = crate::humanized_size(HEAP_SIZE as u64);
        output += format!(
            "Heap   : {:.3} {} used / {:.3} {} free / {:.3} {} total\n",
            used_size, used_unit, free_size, free_unit, total_size, total_unit
        ).as_str();
        
        output += format!("Queue  : {:?}\n", self.ready_queue.lock()).as_str();

        output += &processor::print_processors();

        print!("{}", output);
    }

    pub fn exit_code(&self)->Option<isize>{
        self.current().read().exit_code()
    }

    pub fn spawn(
    &self,
    elf: &ElfFile,
    name: String,
    parent: Option<Weak<Process>>,
    proc_data: Option<ProcessData>,
    ) -> ProcessId {
    let kproc = self.get_proc(&KERNEL_PID).unwrap();
    let page_table = kproc.read().clone_page_table();
    let proc_vm = Some(ProcessVm::new(page_table));
    let proc = Process::new(name, parent, proc_vm, proc_data);

    let entry_point = {
        let mut inner = proc.write();
        inner.load_elf(elf);
        VirtAddr::new(elf.header.pt2.entry_point())
    };

    // Phase 2: Allocate and initialize stack (locks are now free)
    let stack_top = proc.alloc_init_stack();
    
    proc.write().init_stack_frame(entry_point, stack_top);

    trace!("New {:#?}", &proc);

    let pid = proc.pid();
    self.add_proc(pid, proc);
    self.push_ready(pid);
    pid
}

    pub fn app_list(&self) -> Option<&boot::AppList> {
    self.app_list.as_ref()
    }

    pub fn read(&self, fd: u8, buf: &mut [u8]) -> isize {
    let current = self.current();
    let inner = current.read();
    if let Some(data) = &inner.proc_data {
        // ProcessInner 实现了 Deref 到 ProcessData（通过 Process 的 Deref 实现）
        // 但这里需要直接通过 proc_data 字段访问
        data.read(fd, buf)
    } else {
        -1
    }
}

    pub fn write(&self, fd: u8, buf: &[u8]) -> isize {
        let current = self.current();
        let inner = current.read();
        if let Some(data) = &inner.proc_data {
            data.write(fd, buf)
        } else {
            -1
        }
    }

    pub fn block(&self, pid:ProcessId){
        if let Some(proc) = self.get_proc(&pid){
            proc.write().block();
        }
    }
    pub fn get_exit_code(&self, pid: ProcessId) -> Option<isize>{
    let proc = self.get_proc(&pid)?;
    let inner = proc.read();

    if inner.status() == ProgramStatus::Dead{
        inner.exit_code()
    }
    else{
        None
    }
}

    pub fn wait_pid(&self, pid:ProcessId){
        let current= processor::get_pid();

        self.wait_queue
            .lock()
            .entry(pid)
            .or_default()
            .insert(current);
    }

    pub fn wake_up_waiter(&self, pid: ProcessId, ret: isize){
        let waiters = self.wait_queue.lock().remove(&pid);

        if let Some(waiters) = waiters{
            for waiter_pid in waiters{
                if let Some(waiter) = self.get_proc(&waiter_pid){
                    let mut inner = waiter.write();

                    inner.set_return_value(ret as usize);
                    inner.wake_up();

                    drop(inner);

                    self.push_ready(waiter_pid);
                }
            }
        }
    }
}

fn format_usage(name: &str, used: usize, total: usize) -> String {
    let (used_size, used_unit) = crate::humanized_size(used as u64);
    let (total_size, total_unit) = crate::humanized_size(total as u64);
    let percentage = if total == 0 {
        0.0
    } else {
        used as f64 * 100.0 / total as f64
    };

    format!(
        "{:<6} : {:>6.2} {:>3} / {:>6.2} {:>3} ({:>5.2}%)\n",
        name, used_size, used_unit, total_size, total_unit, percentage
    )
}
