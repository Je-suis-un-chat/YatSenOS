#![no_std]
#![no_main]

use yyos::*;
use yyos_kernel as yyos;

extern crate alloc;

boot::entry_point!(kernel_main);

pub fn kernel_main(boot_info: &'static boot::BootInfo) -> ! {
    yyos::init(boot_info);
    yyos::wait(spawn_init());
    yyos::shutdown();
}

pub fn spawn_init() -> proc::ProcessId {
    // NOTE: you may want to clear the screen before starting the shell
    // print!("\x1b[1;1H\x1b[2J");

    proc::list_app();
    proc::spawn("shell").unwrap()
}
