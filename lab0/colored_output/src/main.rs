use colored::*;

fn main() {
    println!("=== 1. 使用现有的 Crate (colored) ===");

    // a. INFO: INFO为绿色，后续白色
    println!("{} {}", "INFO:".green(), "Hello, world!".white());

    // b. WARNING: 黄色，加粗，仅 WARNING 带下划线
    println!(
        "{} {}",
        "WARNING:".yellow().bold().underline(),
        "I'm a teapot!".yellow().bold()
    );

    // c. ERROR: 红色，加粗，居中
    // 小技巧：使用 Rust 自带的格式化参数 {:^80} 可以让字符串在 80 个字符宽度内居中
    let error_msg = "ERROR: KERNEL PANIC!!!";
    let centered_error = format!("{:^80}", error_msg);
    println!("{}", centered_error.red().bold());

    // d. 自定义尝试：背景色 + 反转颜色
    println!("{}", " [DEBUG] Memory dumped successfully ".white().on_blue().bold());


    println!("\n=== 2. 使用直接的 print! 宏与 ANSI 转义序列 ===");

    // ANSI 转义序列语法： \x1b[ {样式代码} m 
    // \x1b[0m 用于重置所有样式，防止颜色污染后面的终端输出

    // a. INFO (32=绿色, 37=白色, 0=重置)
    println!("\x1b[32mINFO:\x1b[0m \x1b[37mHello, world!\x1b[0m");

    // b. WARNING (33=黄色, 1=加粗, 4=下划线, 24=关闭下划线)
    // 我们用分号隔开多个属性，然后在输出后半段前关闭下划线
    println!("\x1b[33;1;4mWARNING:\x1b[24m I'm a teapot!\x1b[0m");

    // c. ERROR (31=红色, 1=加粗)
    // 同样利用 Rust 的居中格式化排版，然后再给整体包裹上红色的 ANSI 代码
    println!("\x1b[31;1m{:^80}\x1b[0m", "ERROR: KERNEL PANIC!!!");

    // d. 自定义尝试 (44=蓝色背景, 37=白色字体, 1=加粗)
    println!("\x1b[44;37;1m [DEBUG] Memory dumped successfully \x1b[0m");
}