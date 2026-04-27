use alloc::{collections::*, format, sync::Arc};
use hashbrown::HashMap;
use spin::{Mutex, RwLock};
use x86::current;
use super::*;
use crate::memory::{
    self, PAGE_SIZE,
    allocator::{ALLOCATOR, HEAP_SIZE},
    get_frame_alloc_for_sure,
};
use crate::utils::macros::*;



pub static PROCESS_MANAGER: spin::Once<ProcessManager> = spin::Once::new();

pub fn init(init: Arc<Process>) {
    // FIXME: set init process as Running
    init.write().resume();
    // FIXME: set processor's current pid to init's pid
    processor::set_pid(init.pid());
    PROCESS_MANAGER.call_once(|| ProcessManager::new(init));
}

pub fn get_process_manager() -> &'static ProcessManager {
    PROCESS_MANAGER
        .get()
        .expect("Process Manager has not been initialized")
}

pub struct ProcessManager {
    processes: RwLock<HashMap<ProcessId, Arc<Process>, ahash::RandomState>>,
    ready_queue: Mutex<VecDeque<ProcessId>>,
}

impl ProcessManager {
    //这个传入的进程会作为内核进程加入进程列表
    pub fn new(init: Arc<Process>) -> Self {
        let mut processes = HashMap::default();
        let ready_queue = VecDeque::new();
        let pid = init.pid();

        trace!("Init {:#?}", init);

        processes.insert(pid, init);
        Self {
            processes: RwLock::new(processes),
            ready_queue: Mutex::new(ready_queue),
        }
    }

    #[inline]
    pub fn push_ready(&self, pid: ProcessId) {
        self.ready_queue.lock().push_back(pid);
    }

    #[inline]
    fn add_proc(&self, pid: ProcessId, proc: Arc<Process>) {
        self.processes.write().insert(pid, proc);
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
        // FIXME: update current process's tick count
        let cur = self.current();
        // FIXME: save current process's context
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


    pub fn spawn_kernel_thread(
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
        // FIXME: set the stack frame
        proc.write().init_stack_frame(entry, stack_top);
        // FIXME: add to process map
        self.add_proc(pid, proc);
        // FIXME: push to ready queue
        self.push_ready(pid);
        // FIXME: return new process pid
        pid
    }

    pub fn kill_current(&self, ret: isize) {
        self.kill(processor::get_pid(), ret);
    }

    pub fn handle_page_fault(&self, addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
        // FIXME: handle page fault
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
        let proc = current.read();
    
        drop(proc);
        if current.write().handle_page_fault(addr) {
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
        let mut output = String::from("  PID | PPID | Process Name |  Ticks  | Status\n");

        self.processes
            .read()
            .values()
            .filter(|p| p.read().status() != ProgramStatus::Dead)
            .for_each(|p| output += format!("{}\n", p).as_str());

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
}
