#![no_std]
#![no_main]

use core::fmt::Arguments;

use lib::*;

extern crate lib;

fn main() -> isize {
    // 清屏
    print!("\x1b[1;1H\x1b[2J");

    println!("YYOS Shell v0.4");
    println!("Type 'help' for available commands.\n");

    loop {
        print!("yyos> ");
        let line = stdin().read_line();
        if line.is_empty() {
            continue;
        }
        execute_command(&line);
    }

    
}

entry!(main);

fn execute_command(cmd: &str) {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return;
    }

    let mut parts = cmd.splitn(2, ' ');
    let command = parts.next().unwrap_or("");
    let argument = parts.next().unwrap_or("");

    match command {
        "help"    => cmd_help(),
        "ls"      => cmd_list_dir(argument),
        "cat"     => cmd_cat(argument),
        "apps"    => cmd_list_apps(),
        "ps"      => cmd_stat(),
        "stat"    => cmd_stat(),
        "run"     => cmd_run(argument),
        "clear"   => cmd_clear(),
        "exit"    => {
            println!("Goodbye!");
            sys_exit(0);
        }
        _         => cmd_run(command), // 未知命令 → 尝试当作程序名运行
    }
}

fn cmd_help() {
    println!("学号：24312063");
    println!("Available commands:");
    println!("  help        - Show this help message");
    println!("  ls / apps   - List all Files / apps");
    println!("  ps / stat   - List all running processes");
    println!("  run <name>  - Run a user program by name");
    println!("  clear       - Clear the screen");
    println!("  exit        - Exit the shell");
    println!("  cat <path>  - Print file contents");
    println!("");
    println!("You can also type a program name directly to run it.");
}

fn cmd_list_apps() {
    let mut buf = [0u8; 512];
    let len = sys_list_app(&mut buf);

    if len == 0 {
        println!("No applications available.");
        return;
    }

    let apps = unsafe { core::str::from_utf8_unchecked(&buf[..len]) };
    println!("Available applications:");
    for app in apps.split('\n') {
        if !app.is_empty() {
            println!("  [app] {}", app);
        }
    }
}

fn cmd_stat() {
    println!("--- Process Status ---");
    sys_stat();
}

fn cmd_run(name: &str) {
    if name.is_empty() {
        println!("Usage: run <program_name>");
        println!("Use 'ls' to see available programs.");
        return;
    }

    let pid = sys_spawn(name);
    if pid == 0 {
        println!("Error: Failed to spawn '{}'.", name);
        println!("Use 'ls' to see available programs.");
        return;
    }

    println!("Running '{}' (PID: {})...", name, pid);

    // 轮询等待子进程结束
    loop {
        let result = sys_wait_pid(pid);
        if result >= 0 {
            println!("Process {} exited with code {}.", pid, result);
            break;
        }
        core::hint::spin_loop();
    }
}

fn cmd_clear() {
    print!("\x1b[1;1H\x1b[2J");
}

fn cmd_list_dir(path: &str) {
    let path = if path.is_empty() { "/" } else { path };

    if !sys_list_dir(path) {
        println!("Failed to list directory '{}'.", path);
    }
}

fn cmd_cat(path: &str) {
    if path.is_empty() {
        println!("Usage: cat <path>");
        return;
    }

    let fd = match sys_open(path) {
        Some(fd) => fd,
        None => {
            println!("cat: cannot open '{}'", path);
            return;
        }
    };

    let mut buf = [0u8; 512];

    loop {
        match sys_read(fd, &mut buf) {
            Some(0) => break,
            Some(count) => {
                if sys_write(1, &buf[..count]).is_none() {
                    println!("cat: output error");
                    break;
                }
            }
            None => {
                println!("cat: read error");
                break;
            }
        }
    }
    println!("");

    sys_close(fd);
}