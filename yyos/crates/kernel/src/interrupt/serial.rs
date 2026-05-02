use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use super::{ack, consts::SERIAL_INTERRUPT_VEC};
use crate::serial::get_serial;

pub fn register_idt(idt: &mut InterruptDescriptorTable) {
    // 依然使用你之前在 consts.rs 里定义的正确的常量
    idt[SERIAL_INTERRUPT_VEC].set_handler_fn(serial_handler);
}

pub extern "x86-interrupt" fn serial_handler(_st: InterruptStackFrame) {
    receive(); // 调用下方的接收逻辑
    ack();     // 结束中断
}

/// 仅仅从 UART 读取字符，放入 INPUT_BUFFER (Top Half)
fn receive() {
    if let Some(mut serial) = get_serial() {
        while let Some(byte) = serial.receive() {
            // 将收到的字节推入刚刚写好的缓冲队列
            crate::drivers::input::push_key(byte);
        }
    }
}