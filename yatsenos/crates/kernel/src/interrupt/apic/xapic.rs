use core::{
    fmt::{Debug, Error, Formatter},
    ptr::{read_volatile, write_volatile},
};

use bit_field::BitField;
use x86::cpuid::CpuId;

use super::LocalApic;

/// Default physical address of xAPIC
pub const LAPIC_ADDR: u64 = 0xFEE00000;

pub struct XApic {
    addr: u64,
}

impl XApic {
    pub unsafe fn new(addr: u64) -> Self {
        XApic { addr }
    }

    unsafe fn read(&self, reg: u32) -> u32 {
        unsafe { read_volatile((self.addr + reg as u64) as *const u32) }
    }

    unsafe fn write(&mut self, reg: u32, value: u32) {
        unsafe {
            write_volatile((self.addr + reg as u64) as *mut u32, value);
            self.read(0x20);
        }
    }
}

impl LocalApic for XApic {
    /// If this type APIC is supported
    fn support() -> bool {
        // FIXME: Check CPUID to see if xAPIC is supported.
        CpuId::new()
            .get_feature_info()
            .map(|f| f.has_apic())
            .unwrap_or(false)
    }

    /// Initialize the xAPIC for the current CPU.
    fn cpu_init(&mut self) {
        unsafe {
          // 1. 启用 Local APIC 并设置伪中断向量 (Spurious Interrupt Vector)
            // 寄存器 0xF0: 位 8 是软件启用位，0-7 是向量号
            let spurious_vector = 0xFF; // 通常使用 0xFF 作为伪中断向量
            self.write(0xF0, spurious_vector | (1 << 8));

            // 2. 配置 LVT Timer (时钟)
            // 寄存器 0x3E0: 设置分频器。0x0B (1011b) 表示 1分频
            self.write(0x3E0, 0x0B);
            
            // 寄存 crate::interrupts::consts 里的向量号
            let timer_vec = 0x20; // 假设时钟向量号为 0x20
            // 寄存器 0x320: 位 17:18 为模式 (01b 是 Periodic)，位 16 是屏蔽位 (0 表示开启)
            self.write(0x320, timer_vec | (1 << 17)); 
            
            // 寄存器 0x380: 设置初始计数值，计数到 0 时触发中断
            self.write(0x380, 1000000); 

            // 3. 禁用不需要的 LVT 线路 (LINT0, LINT1, PCINT, Error)
            // 将这些寄存器的位 16 (Mask) 设为 1
            self.write(0x350, 1 << 16); // LINT0
            self.write(0x360, 1 << 16); // LINT1
            self.write(0x340, 1 << 16); // Performance Counter
            
            // 将错误中断映射到特定向量并启用
            let error_vec = 0x31; // 假设错误中断向量为 0x31
            self.write(0x370, error_vec); 

            // 4. 清除错误状态寄存器 (ESR)
            // 必须连续写入两次才能清除
            self.write(0x280, 0);
            self.write(0x280, 0);

            // 5. 确认并清除所有挂起的中断 (EOI)
            self.eoi();

            // 6. 发送 Init Level De-assert 信号 (同步仲裁 ID)
            // 这是为了在多核环境下同步 APIC 状态
            self.write(0x310, 0); // 写 ICR 高 32 位 (目标 CPU 为 0)
            // 写 ICR 低 32 位: Level De-assert (位 15=0), All Excl Self (位 18:19=11b)
            self.write(0x300, 0x000C8500); 
            while self.read(0x300) & (1 << 12) != 0 {
                core::hint::spin_loop();
            }

            // 7. 设置任务优先级寄存器 (TPR)
            // 允许所有优先级的中断进入 CPU
            self.write(0x080, 0);
        }

        // NOTE: Try to use bitflags! macro to set the flags.
    }

    fn id(&self) -> u32 {
        // NOTE: Maybe you can handle regs like `0x0300` as a const.
        unsafe { self.read(0x0020) >> 24 }
    }

    fn version(&self) -> u32 {
        unsafe { self.read(0x0030) }
    }

    fn icr(&self) -> u64 {
        unsafe { (self.read(0x0310) as u64) << 32 | self.read(0x0300) as u64 }
    }

    fn set_icr(&mut self, value: u64) {
        unsafe {
            while self.read(0x0300).get_bit(12) {}
            self.write(0x0310, (value >> 32) as u32);
            self.write(0x0300, value as u32);
            while self.read(0x0300).get_bit(12) {}
        }
    }

    fn eoi(&mut self) {
        unsafe {
            self.write(0x00B0, 0);
        }
    }
}

impl Debug for XApic {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.debug_struct("Xapic")
            .field("id", &self.id())
            .field("version", &self.version())
            .field("icr", &self.icr())
            .finish()
    }
}
