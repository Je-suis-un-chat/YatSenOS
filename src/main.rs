use std::fs;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

// a. 倒计时函数
fn count_down(seconds: u64) {
    for i in (1..=seconds).rev() {
        println!("剩余时间: {} 秒", i);
        thread::sleep(Duration::from_secs(1));
    }
    println!("Countdown finished!");
}

// b. 读取并输出文件内容 (使用 ? 传播错误)
fn read_and_print(file_path: &str) -> io::Result<()> {
    let content = fs::read_to_string(file_path)?;
    println!("文件内容如下:\n{}", content);
    Ok(())
}

// c. 获取文件大小 (使用 map_err 转换错误类型)
fn file_size(file_path: &str) -> Result<u64, &str> {
    fs::metadata(file_path)
        .map_err(|_| "File not found!") // 将 io::Error 转换为指定的字符串
        .map(|meta| meta.len())        // 如果成功，返回大小
}

// d. 主函数
fn main() -> io::Result<()> {
    // 1. 倒计时
    count_down(5);

    // 2. 读取指定文件
    println!("\n--- 尝试读取 /etc/hosts ---");
    // 这里使用 expect 遵循图片 b 的初始要求，如果文件不存在则崩溃
    read_and_print("/etc/hosts").expect("File not found!");

    // 3. 获取用户输入并查询文件大小
    println!("\n--- 获取文件大小服务 ---");
    loop {
        print!("请输入文件路径 (输入 'q' 退出): ");
        io::stdout().flush()?; // 确保提示文字立即显示

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let path = input.trim();

        if path == "q" { break; }

        match file_size(path) {
            Ok(size) => println!("文件 '{}' 的大小为: {} 字节", path, size),
            Err(e) => println!("错误: {}", e),
        }
    }

    Ok(())
}