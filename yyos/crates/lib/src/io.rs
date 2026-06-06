use alloc::{
    string::{String, ToString},
    vec,
};

use crate::*;

pub struct Stdin;
pub struct Stdout;
pub struct Stderr;

impl Stdin {
    fn new() -> Self {
        Self
    }

    pub fn read_line(&self) -> String {
        let mut s = String::with_capacity(128);
        let mut buf = [0u8; 1];

        loop {
            let n = sys_read(0, &mut buf);
            if n == core::prelude::v1::Some(0) {
                // 没有输入可用，短暂等待后继续轮询
                // 调度器会通过时钟中断切换进程，不会死锁
                core::hint::spin_loop();
                continue;
            }

            let ch = buf[0];
            match ch {
                b'\r' | b'\n' => {
                    // 回车/换行 → 结束输入，回显换行
                    sys_write(1, b"\n");
                    return s;
                }
                0x08 | 0x7F => {
                    // 退格键 → 删除最后一个字符，回显退格效果
                    if !s.is_empty() {
                        s.pop();
                        sys_write(1, b"\x08 \x08");
                    }
                }
                0x20..=0x7E => {
                    // 可打印 ASCII → 加入字符串并回显
                    s.push(ch as char);
                    sys_write(1, &[ch]);
                }
                _ => {
                    // 忽略其他控制字符
                }
            }
        }
    }
}

impl Stdout {
    fn new() -> Self {
        Self
    }

    pub fn write(&self, s: &str) {
        sys_write(1, s.as_bytes());
    }
}

impl Stderr {
    fn new() -> Self {
        Self
    }

    pub fn write(&self, s: &str) {
        sys_write(2, s.as_bytes());
    }
}

pub fn stdin() -> Stdin {
    Stdin::new()
}

pub fn stdout() -> Stdout {
    Stdout::new()
}

pub fn stderr() -> Stderr {
    Stderr::new()
}
