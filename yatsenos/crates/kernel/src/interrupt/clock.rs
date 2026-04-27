use super::consts::*;
// 引入原子类型和内存排序规则
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::as_handler;
use crate::proc::{switch,ProcessContext};
use crate::memory::gdt::CLOCK_IST_INDEX;


as_handler!(clock);
pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt[Interrupts::IrqBase as u8 + Irq::Timer as u8]
        .set_handler_fn(clock_handler)
        .set_stack_index(CLOCK_IST_INDEX);
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

#[inline]
pub fn read_counter() -> u64 {
    // 使用 Relaxed 内存序读取当前值
    COUNTER.load(Ordering::Relaxed)
}

#[inline]
pub fn inc_counter() -> u64 {
    // fetch_add 会将变量加 1，并返回相加【之前】的旧值。
    // 为了让 inc_counter 返回最新的值，我们在后面 + 1
    COUNTER.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn clock(context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        /*if inc_counter() % 0x100 == 0 {
            info!("Tick! @{}", read_counter());
        }*/
        switch(context);
        super::ack();
    });
}


