mod apic;
pub mod clock;
mod consts;
mod exceptions;
mod keyboard;
mod serial;
pub mod syscall;

use apic::*;
use x86_64::structures::idt::InterruptDescriptorTable;

use crate::memory::physical_to_virtual;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            exceptions::register_idt(&mut idt);
            clock::register_idt(&mut idt);
            serial::register_idt(&mut idt);
            keyboard::register_idt(&mut idt);
            syscall::register_idt(&mut idt);
        }
        idt
    };
}

/// init interrupts system
pub fn init() {
    IDT.load();

    unsafe {
        //初始化本地APIC
        //映射物理地址到虚拟地址并初始化

        let mut lapic = XApic::new(physical_to_virtual(LAPIC_ADDR));
        lapic.cpu_init();

        //启用串口中断
        //串口COM1通常对应 IOAPIC 的 IRQ4
        //将其路由至CPU 0(主核)
        enable_irq(4,0);
        
        //启用键盘和时钟中断
        enable_irq(1,0);
        enable_irq(0,0);
        //开启CPU的中断响应开关
        //x86_64::instructions::interrupts::enable();
    }

    info!("Interrupts Initialized.");
}

#[inline(always)]
pub fn enable_irq(irq: u8, cpuid: u8) {
    let mut ioapic = unsafe { IoApic::new(physical_to_virtual(IOAPIC_ADDR)) };
    ioapic.enable(irq, cpuid);
}

#[inline(always)]
pub fn ack() {
    let mut lapic = unsafe { XApic::new(physical_to_virtual(LAPIC_ADDR)) };
    lapic.eoi();
}
