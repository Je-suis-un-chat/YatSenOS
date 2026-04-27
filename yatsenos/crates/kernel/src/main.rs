#![no_std]
#![no_main]

use ysos::*;
use ysos_kernel as ysos;
use log::info;

extern crate alloc;

boot::entry_point!(kernel_main);

pub fn kernel_main(boot_info: &'static boot::BootInfo) -> ! {
    ysos::init(boot_info);
    
    info!("开始创建进程：");
    // 自动创建 5 个测试进程，观察并发调度效果
    for i in 0..5 {
        ysos::new_test_thread(format!("{}", i).as_str());
    }
    info!("Created 5 test threads, scheduler is running...");

    // 后续手动创建的测试进程从 5 开始编号
    let mut test_num = 5;

    loop {
        print!("[>] ");
        let line = input::get_line();
        match line.trim() {
            "exit" => break,
            "ps" => {
                ysos::proc::print_process_list();
            }
            "stack" => {
                ysos::new_stack_test_thread();
            }
            "test" => {
                ysos::new_test_thread(format!("{}", test_num).as_str());
                test_num += 1;
            }
            _ => println!("[=] {}", line),
        }
    }

    ysos::shutdown();
}
