#import "../template/report.typ": *
#show raw.where(block: true): set block(breakable: true)

#show: report.with(
  title: "操作系统实验报告",
  subtitle: "实验二：中断处理",
  name: "郭盈盈",
  stdid: "24312063",
  classid: "吴岸聪老师班",
  major: "保密管理",
  school: "计算机学院",
  time: "2025 学年第二学期",
  banner: "./images/sysu.png"
)

= 实验目的

1. 了解中断的作用、中断的分类、中断的处理过程。

2. 启用基于 APIC 的中断，注册 IDT 中断处理程序，实现时钟中断。

3. 注册内核堆分配器。（不实现内存分配算法，使用现有代码赋予内核堆分配能力）

4. 实现串口驱动的输入能力，尝试进行基础的 IO 操作和交互。

= 实验内容

1. 合并实验代码

2. GDT与TSS

3. 注册中断处理程序

4. 初始化APIC

5. 时钟中断

6. 串口输入中断

7. 用户交互

= 实验过程

== 合并实验代码

为了保持每个实验的相对独立性同时实现代码的继承性，我在原有的实验文件夹上新建了一个Git分支，并将实验二的代码和上一次实验的代码在新分支上进行合并。这样可以保证实验的连贯性，同时可以随时回退到完成某一次实验时的状态。

值得注意的是，我的整个实验文件夹是YatSenOS,具体的实验代码在yatsenos文件夹下，一开始我是在YatSenOS中进行```bash cp```命令的，导致文件结构完全乱套。于是我用```bash rm```命令将拷错的文件删掉，再在子目录中把官方代码拉进来。

这一步可以用```bash git status ```监测到：

```bash
(base) je-suis-un-chat@LAPTOP-MAGCR3QA:~/YatSenOS$ git status
On branch lab2
Your branch is up to date with 'origin/lab2'.

Changes not staged for commit:
  (use "git add <file>..." to update what will be committed)
  (use "git restore <file>..." to discard changes in working directory)
        modified:   yatsenos/Cargo.lock
        modified:   yatsenos/crates/kernel/src/memory/gdt.rs

Untracked files:
  (use "git add <file>..." to include in what will be committed)
        yatsenos/lab2_report.typ
```
现在，我可以基于上个实验和官方文档的代码进一步进行操作系统的开发了。

== GDT 与 TSS 

=== 步骤一：知识学习

==== 全局描述符表(GDT)

全局描述符表是x86架构下的一种数据结构，用于存储段描述符、定义和管理内存段的访问权限和属性。它是操作系统内核与硬件之间的桥梁，允许操作系统精确地控制不同段的访问权限。

在整个操作系统中，全局描述符表 GDT 只有一张(一个处理器对应一个GDT)，GDT可以被放在内存的任何位置，但CPU必须知道GDT的入口，也就是知道基地址放在哪里，intel设计者们提供了一个寄存器GDTR用来存放GDT的入口地址，程序员将GDT设定在内存中某个位置之后，可以通过LGDT指令将GDT的入口地址装入此寄存器。从此以后，CPU就根据此寄存器中的内容作为GDT的入口来访问GDT了。

==== 任务状态段(TSS)

任务状态段(Task State Segment,TSS) 也是一种数据结构，它存储有关任务的相关数据。

在32位保护模式下，它主要用于存储与任务和中断处理相关的信息。TSS包含了处理器在任务切换时需要保护和恢复的一些状态信息。每个任务都有一个相应的TSS，通过任务寄存器(TR)来引用。

在64位长模式下，TSS的结构与32位不同，它并不直接与任务切换挂钩，但是它仍然被用于存储特权级栈和中断栈。

==== 中断描述符表(IDT)

中断描述符表(Interrupt Descriptor Table,IDT)是用于存储中断门描述符的数据结构。

使用IDT时，需要为每个可能的中断分配一个唯一的中断门描述符，对于x86_64架构，前32个中断号被intel保留，用于处理CPU异常，之后的描述符是用户自定义的中断处理程序，可以被操作系统自定义。

中断门描述符中的地址字段指向相应中断处理程序的入口地址、中断栈等信息。在中断发生时，中断上下文的信息将会被保留在中断栈中，并将处理程序的地址放置到RIP寄存器中来进行调用。这一过程遵守了x86_64的相关调用约定，因此中断处理程序也需要遵守相关的ABI约定，在x86_64中，应当通过```bash iretq ```指令来结束中断调用。

=== 步骤二：参考上下文，在 src/memory/gdt.rs中补全 TSS 的中断栈表，代码如下：

```rs
lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();

        // initialize the TSS with the static buffers
        // will be allocated on the bss section when the kernel is load
        //
        // DO NOT MODIFY THE FOLLOWING CODE
        tss.privilege_stack_table[0] = {
            const STACK_SIZE: usize = IST_SIZES[0];
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            let stack_start = VirtAddr::from_ptr(addr_of_mut!(STACK));
            let stack_end = stack_start + STACK_SIZE as u64;
            info!(
                "Privilege Stack  : 0x{:016x}-0x{:016x}",
                stack_start.as_u64(),
                stack_end.as_u64()
            );
            stack_end
        };
        // 设置 Double Fault 专用栈
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = IST_SIZES[1];
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            let stack_start = VirtAddr::from_ptr(unsafe { addr_of_mut!(STACK) });
            let stack_end = stack_start + STACK_SIZE as u64;
            info!(
                "Double Fault IST : 0x{:016x}-0x{:016x}",
                stack_start.as_u64(),
                stack_end.as_u64()
            );
            stack_end
        };
         ```
        ```rs
        // 设置 Page Fault 专用栈
        tss.interrupt_stack_table[PAGE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = IST_SIZES[2];
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            let stack_start = VirtAddr::from_ptr(unsafe { addr_of_mut!(STACK) });
            let stack_end = stack_start + STACK_SIZE as u64;
            info!(
                "Page Fault IST   : 0x{:016x}-0x{:016x}",
                stack_start.as_u64(),
                stack_end.as_u64()
            );
            stack_end
        };

        tss
    };
}

```

== 注册中断处理程序

=== 在exception.rs中,为各种CPU异常注册中断处理程序并完成中断处理函数

- 注册中断处理程序：

```rs
pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt.divide_error.set_handler_fn(divide_error_handler);
    idt.double_fault
        .set_handler_fn(double_fault_handler)
        .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
    idt.page_fault
        .set_handler_fn(page_fault_handler)
        .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
    
    // TODO: you should handle more exceptions here
    // especially general protection fault (GPF)
    // see: https://wiki.osdev.org/Exceptions

    idt.general_protection_fault.set_handler_fn(general_protection_fault_handler)
    .set_stack_index(gdt::GENERAL_PROTECTION_FAULT_IST_INDEX);
    
    idt.breakpoint.set_handler_fn(breakpoint_handle);

    idt.invalid_opcode.set_handler_fn(invalid_opcode_handle);
}
```

- 完成中断处理函数：

```rs
fn parse_gp_error(code: u64) -> &'static str{
    if code == 0{
        "Null Selector or General Violation"
    }else if code & 0b1 != 0{
        "External Event (Hardware)"
    }else {
        match  (code >>1)& 0b11 {
            0b00 => "GDT Violation",
            0b01 => "IDT Violation",
            0b10 => "LDT Violation",
            0b11 => "IDT Violation (with TI set)",
            _ => "Unknown Segment Violation",
        }
    }
}

pub extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    err_code: u64,
){
    panic!(
        "EXCEPTION: GENERAL_PROTECTION_FAULT,ERROR_CODE: {:#x}\n\nDescription: {}\n\nStack_Frame: {:#?}",
        err_code,
        parse_gp_error(err_code),
        stack_frame
    );
}

pub extern "x86-interrupt" fn breakpoint_handle(stack_frame: InterruptStackFrame){
        info!("EXCEPTION: BREAKPOINT\n\nStack_Frame: {:#?}",stack_frame)  
    }

pub extern "x86-interrupt" fn invalid_opcode_handle(stack_frame: InterruptStackFrame){
    panic!(
        "EXCEPTION: INVALID_OPCODE\n\nStack_Frame: {:#?}",
        stack_frame
    );
}
```

特别地,由于这一步中为General protection Fault 设置了专用栈，比如修改 gdt.rs 文件,在TSS中补全内存分配。

```rs
pub const GENERAL_PROTECTION_FAULT_IST_INDEX: u16 = 2;

tss.interrupt_stack_table[GENERAL_PROTECTION_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = IST_SIZES[3];
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            let stack_start = VirtAddr::from_ptr(unsafe {
                addr_of_mut!(STACK)
            };)
            let stack_end = stack_start + STACK_SIZE as u64;
            info!(
                "General Protection Fault IST  :0x{:016x}-0x{:016x}",
                stack_start.as_u64(),
                stack_end.as_u64()
            );
            stack_end
        };
```

===== 初始化中断系统

```rs
pub fn init() {
    IDT.load();

    unsafe {
        //初始化本地APIC
        //映射物理地址到虚拟地址并初始化

        let mut lapic = XApic::new(physical_to_virtual(LAPIC_ADDR));
        lapic.init();

        //启用串口中断
        //串口COM1通常对应 IOAPIC 的 IRQ4
        //将其路由至CPU 0(主核)
        enable_irq(4,0);
        
        //启用时钟中断
        enable_irq(0,0);
        //开启CPU的中断响应开关
        x86_64::instructions::interrupts::enable();
    }

    info!("Interrupts Initialized.");
}
```

== 初始化APIC 

=== 步骤一：知识学习之APIC可编程中断控制器

1. 什么是APIC

APIC(高级可编程中断控制器)是一种关键的硬件组件，旨在管理和协调系统内的中断请求。

APIC 不仅简单地分配中断向量，还提供了更为复杂的功能，如中断优先级、中断屏蔽、中断向量分发等。这使得它成为多处理器系统中协调中断处理的理想选择，并在大型、高性能的计算机系统中发挥关键作用。APIC 的作用不仅仅局限于中断处理，它还协助处理器间通信、同步和系统管理。通过提供多处理器系统中的高级中断控制和协同工作机制，APIC 极大地推动了操作系统和应用程序在复杂环境下的性能表现。

2. APIC 的初始化与编程

在基于APIC的系统中，每个CPU都由一个本地APIC(LAPIC)控制。LAPIC 通过MMIO方式映射到物理内存中的某个地址空间，这个地址空间称为LAPIC寄存器空间。

同时，系统中还有一个I/O APIC，它是一个独立芯片，负责管理系统中所有I/O设备的中断请求。I/O APIC也通过MMIO方式映射到物理内存中的某个地址空间。

x2APIC 是 xAPIC 的变体和扩展，主要改进解决了支持的 CPU 数量和接口性能问题，它们都属于 LAPIC 的实现。在本实验中，我们将使用 xAPIC 来实现 LAPIC 的初始化和编程，在之后的描述中，出现的 APIC 均代指 xAPIC。

=== 步骤二：补全检查和初始化函数代码

- 补全 support 检查：

```rs
impl LocalApic for XApic {
    fn support() -> bool {
        CpuId::new()
            .get_feature_info()
            .map(|f| f.has_apic())
            .unwrap_or(false)
    }
    // ...
}
```

- 补全cpu_init初始化逻辑

```rs
fn cpu_init(&mut self) {
        unsafe {
            // 1. 启用 Local APIC 并设置伪中断向量 (Spurious Interrupt Vector)
            // 寄存器 0xF0: 位 8 是软件启用位，0-7 是向量号
            let spurious_vector = 0xFF; // 通常使用 0xFF 作为伪中断向量
            self.write(0xF0, spurious_vector | (1 << 8));
```
```rs

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
    }
```
在上次实验中，由于内核没有内存分配器无法编译通过，我在main.rs中定义了一个虚假的分配器```rs DummyAllocator ```,.本次实验中，lib.rs 已经调用了```rs memory::allocator::init() ```,意味着其已经实现了一个真正的、可用的全局堆内存分配器。于是在```bash make run ```之前，我需要把之前的```rs DummyAllocator ```相关代码注释掉，这样代码就运行成功了，结果如下：

#figure(
    image("img/init_apic1.png",width: 120%),
    
)

== 时钟中断

=== 步骤一：创建 src/interrupt/clock.rs 文件，补全代码，为 Timer 设置中断处理程序：

```rs
use super::consts::*;
// 引入原子类型和内存排序规则
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt[Interrupts::IrqBase as u8 + Irq::Timer as u8]
        .set_handler_fn(clock_handler);
}

pub extern "x86-interrupt" fn clock_handler(_sf: InterruptStackFrame) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        if inc_counter() % 0x10000 == 0 {
            info!("Tick! @{}", read_counter());
        }
        super::ack();
    });
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
```
实验结果：

程序每隔一个固定的时间间隔会输出：

```bash
[ INFO] [crates/kernel/src/interrupt/clock.rs:14] Tick! @65536
```

=== 步骤二：回答问题

如果想要中断的频率减半，应该如何修改？

答：设置一个计数变量count，当 count % 2 == 0 时，触发需要执行的定时任务。再修改日志打印的频率，将原本的```rs inc_counter() % 0x10000 == 0 ``` 改为```rs inc_counter() % 0x20000 == 0 ```。

== 串口输入中断

=== 步骤一：创建 crates/kernel/src/drivers/input.rs 文件 

```rs
use alloc::string::String;
use crossbeam_queue::ArrayQueue;

// 串口输入的是 ASCII 字节
pub type Key = u8; 

lazy_static! {
    static ref INPUT_BUF: ArrayQueue<Key> = ArrayQueue::new(128);
}

#[inline]
pub fn push_key(key: Key) {
    if INPUT_BUF.push(key).is_err() {
        warn!("Input buffer is full. Dropping key '{:?}'", key);
    }
}

#[inline]
pub fn try_pop_key() -> Option<Key> {
    INPUT_BUF.pop()
}

// 3. 实现阻塞式的 pop_key
pub fn pop_key() -> Key {
    loop {
        if let Some(key) = try_pop_key() {
            return key;
        }
        // 如果没有数据，使用 cpu 提示指令降低功耗，自旋等待
        core::hint::spin_loop(); 
    }
}


// 4. 实现 get_line 函数（处理回车和退格）
pub fn get_line() -> String {
    let mut s = String::with_capacity(128);
    loop {
        let key = pop_key();
 ```
```rs
        match key {
            b'\r' | b'\n' => {
                // 遇到回车/换行：回显换行，并返回字符串
                if let Some(mut serial) = crate::serial::get_serial() {
                    serial.send(b'\n'); 
                }
                return s;
            }
            0x08 | 0x7F => {
                // 退格键（Backspace 或 Delete）
                if !s.is_empty() {
                    s.pop(); // 从字符串中删除
                    // 屏幕回显删除：退格(0x08) -> 打印空格盖住 -> 再退格
                    if let Some(mut serial) = crate::serial::get_serial() {
                        serial.send(0x08);
                        serial.send(b' ');
                        serial.send(0x08);
                    }
                }
            }
            _ => {
                // 普通字符：追加到字符串并回显
                s.push(key as char);
                if let Some(mut serial) = crate::serial::get_serial() {
                    serial.send(key);
                }
            }
        }
    }
}
```

=== 步骤二：实现串口中断逻辑

```rs
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
```
```rs
/// 仅仅从 UART 读取字符，放入 INPUT_BUFFER (Top Half)
fn receive() {
    if let Some(mut serial) = get_serial() {
        while let Some(byte) = serial.receive() {
            // 将收到的字节推入刚刚写好的缓冲队列
            crate::drivers::input::push_key(byte);
        }
    }
}
```

=== 步骤三：开启硬件设备的串口中断

在串口初始化的尾部添加：

```rs
let mut ier_port: Port<u8> = Port::new(0x3F8 + 1);
unsafe {
    ier_port.write(0x01); // 开启 Data Available Interrupt
}
```

== 用户交互

修改 crates/kernel/src/main.rs ：

```rs
#![no_std]
#![no_main]

use ysos::*;
use ysos_kernel as ysos;

extern crate alloc;

boot::entry_point!(kernel_main);

pub fn kernel_main(boot_info: &'static boot::BootInfo) -> ! {
    ysos::init(boot_info);

    loop {
        print!("> ");
        let input = input::get_line();
```
```rs
        match input.trim() {
            "exit" => break,
            _ => {
                println!("You said: {}", input);
                println!("The counter value is {}", interrupt::clock::read_counter());
            }
        }
    }

    ysos::shutdown();
}
```

实验结果：

#figure(
    image("img/user_interact.png",width: 120%)
)

== 思考题

=== 为什么需要在 clock_handler 中使用 without_interrupts 函数？如果不使用它，可能会发生什么情况？

答：在clock_handler中使用without_interrupts的根本原因是为了创建一个“临界区”，防止嵌套中断引发的并发灾难。

简单来说，without_interrupts会在它执行期间向CPU发送指令，强行屏蔽掉所有其他的外部硬件中断。等里面的代码执行完，再把中断开关恢复原样。

如果不适用它，内核可能会发生以下三种情况：

1. 经典死锁 (Deadlock)：内核直接卡死

这是在编写 Rust OS（如 YatSenOS 或 Phil Opp 的 blog_os）时最常遇到的崩溃原因。

clock_handler 里调用了 info!("Tick!...")。在内核中，为了防止多行日志打印时字符互相穿插，info! 宏底层一定会使用一个自旋锁（Spinlock）来锁住串口设备（Serial Port）。

如果不关中断，考虑以下灾难场景：

主程序（或者另一个优先级低的中断）正在打印日志，它刚刚获取了串口的锁。

就在这时，时钟中断（IRQ0）触发了！CPU 立刻暂停主程序，跳进 clock_handler。

在 clock_handler 中，也调用了 info!，试图获取同一个串口的锁。

死锁诞生： clock_handler 发现锁被占用了，于是开始死循环等待（自旋）；但占用锁的主程序必须等 clock_handler 执行完才能恢复运行去释放锁。两者永远互相等待，整个操作系统瞬间卡死。

2. 数据竞争与状态破坏 (Data Race)

 clock_handler 中调用了 super::ack() 来告诉中断控制器（APIC 或 PIC）本次中断处理完毕，同时也可能修改其他全局变量。

如果中断没有被屏蔽，当时钟处理函数执行到一半时，突然键盘被按下了，触发了键盘中断。键盘中断的处理程序如果刚好也要读取/修改与时钟共享的内核状态（比如某些时间戳记录、进程调度队列），两边的数据就会交叉覆盖，导致难以复现的幽灵 Bug。

3. 栈溢出 (Stack Overflow)

虽然时钟中断的频率很高，但处理速度通常也很快。然而，如果中断不被屏蔽：
如果在时钟中断还没处理完的时候，又来了一个新的中断（甚至是下一个时钟中断），CPU 就会把当前的状态再次压入栈中，去执行新的中断。
如果系统负载极高，中断像俄罗斯套娃一样无限嵌套，内核栈（通常只有十几 KB）很快就会被耗尽，导致系统直接崩溃引发 Double Fault（双重异常）。

=== 考虑时钟中断进行进程调度的场景，时钟中断的频率应该如何设置？太快或太慢的频率会带来什么问题？请分别回答。

答：

1. 时钟中断频率应该如何设置？

时钟中断频率没有一个“放之四海而皆准”的固定值，它取决于操作系统的应用场景。

权衡原则： 设置频率的本质是在 “任务执行效率” 与 “交互响应速度” 之间寻找平衡点。

常见范围：

桌面系统（如 Ubuntu, Windows）： 通常设置为 250Hz - 1000Hz（即每 1ms 到 4ms 触发一次中断）。高频率能保证鼠标点击、窗口拖动等交互操作非常流畅。

服务器系统： 通常设置为 100Hz（每 10ms 触发一次）。服务器更看重吞吐量，稍微牺牲一点响应时间以换取更高的计算效率。

嵌入式/实时系统： 可能根据具体硬件实时性要求设置得更高（如 1000Hz+），以确保任务在严格的截止时间内完成。

2. 频率设置“太快”（太高）会带来什么问题？

如果时钟中断频率过高（例如设置到 10000Hz，即每 0.1ms 一次中断），会产生以下严重后果：

巨大的 CPU 开销（Overhead）： 每次中断发生时，CPU 都必须强制停止当前工作，保存寄存器现场，进入内核态执行中断处理程序。如果频率太快，CPU 会浪费大量比例的时间在“进出中断”这件事上，而不是执行有意义的用户代码。

上下文切换频繁： 高频时钟意味着时间片（Time Slice）更短，进程会被频繁切换。频繁的上下文切换会导致 CPU 缓存（L1/L2 Cache）污染，因为刚缓存好的数据还没怎么用，进程就被切走了，新进程又要重新加载缓存，大幅降低执行效率。

功耗增加（电量杀手）： 对于移动设备或笔记本，频繁唤醒 CPU 处理中断会阻止 CPU 进入深度的休眠省电模式，导致电池寿命急剧缩短。

3. 频率设置“太慢”（太低）会带来什么问题？

如果频率过低（例如设置为 10Hz，即每 100ms 才一次中断），系统会表现得非常“迟钝”：

交互响应延迟（Latency）： 假设你点击了一下鼠标，系统可能需要等当前这个漫长的 100ms 时钟周期结束才能触发调度器处理你的点击。用户会感觉到明显的“卡顿”或输入延迟。

进程调度粒度太粗： 调度器对进程运行时间的控制变得非常不精确。比如一个进程只需要运行 5ms，但由于 100ms 才有一次“检查点”，它可能会平白无故占用 CPU 很久，导致其他急需运行的小任务排队等待。

系统时间精度下降： 依赖时钟滴答计数的计时器（如 sleep 函数）会变得很不准。如果你想睡眠 10ms，但在 100ms 一跳的系统里，你可能不得不睡满 100ms 才能被唤醒。

=== 在进行 receive 操作的时候，为什么无法进行日志输出？如果强行输出日志，会发生什么情况？谈谈你对串口、互斥锁的认识。

答： 

一、 为什么不能在 receive 中输出日志？强行输出会怎样？

在你的 YatSenOS 中，info! 宏的作用是将字符串打印到屏幕或终端，它的底层必然调用了串口设备（Serial Port）的发送功能。这意味着，串口不仅负责接收键盘输入，还负责输出内核日志，它是一个全局共享的硬件资源。

为了防止多条日志同时打印导致字符乱码拼凑，内核通常会用一个全局互斥锁（在 no_std 内核中通常是自旋锁 Spinlock）将串口保护起来。

如果你在 receive 函数中强行调用 info!，几乎必然会触发以下“死亡连环套”：

场景预设： 内核主程序正在愉快地运行，此时恰好执行到一句普通的 info!("System running...")。

获取锁： 主程序成功获取了串口的全局锁，准备开始向硬件端口发送字符。

灾难降临（中断打断）： 就在主程序刚发了两个字符时，你敲击了一下键盘。串口硬件立刻向 CPU 发送 IRQ4 中断请求。

强行挂起： CPU 响应硬件中断，立刻挂起主程序（此时主程序依然紧紧握着串口的锁没有释放），强行跳入你的 serial_handler，进而调用 receive。

死锁爆发： 在 receive 函数内部，你强行写了一句 info!("Received a key")。info! 宏为了打印，试图去获取那个全局的串口锁。

万劫不复的死循环： receive 发现锁被占用了，于是开始原地死循环（自旋）等待锁被释放；然而，唯一能释放这个锁的主程序，正被 receive 所在的中断上下文无情地压制着，永远得不到 CPU 时间去执行解锁。

结论： 强行输出会引发内核级死锁（Deadlock），整个操作系统瞬间卡死，画面定格，连 Panic 的机会都不会有。

二、 对“串口（Serial Port）”的认识

在现代编程中，我们习惯了极快的 I/O 和无限的缓冲区，但在写 OS 底层驱动时，串口暴露出硬件最原始的模样：

极其缓慢的字符流： 串口是串行通信，一个比特一个比特地发。相对于 CPU 动辄 GHz 的时钟频率，串口的速度慢得像蜗牛。这意味着 CPU 如果要等待串口发完一段长日志，会浪费海量的时钟周期。

极小硬件 FIFO 缓冲区： UART 16550 芯片内部通常只有一个 16 字节的 FIFO（先进先出队列）。如果 CPU 不赶紧把收到的字节读走，或者串口被其他高优先级任务霸占太久，后续敲击的字符就会直接被硬件丢弃（Overrun Error）。

既是矛也是盾的双重身份： 如前文所述，它既负责输入也负责输出。这使得它成为了内核中最容易发生冲突的瓶颈点。

这也就是为什么实验指导书让你使用 Top Half & Bottom Half（上半部与下半部） 原则：
在中断里（Top Half）绝不恋战，不打印日志，甚至不作逻辑处理，以最快的速度把硬件 FIFO 里的字节捞出来，扔进你刚刚写的无锁队列 crossbeam_queue 中就赶紧退出。复杂的打印和回显（Bottom Half），交给普通的内核线程去慢慢做。

三、 对“互斥锁（Mutex / Spinlock）”的认识

普通的 Mutex（如标准库中的 std::sync::Mutex）在获取不到锁时，会让当前线程休眠，让出 CPU 给别的线程。

但在内核中断里，绝对不能休眠！ 中断没有自己的独立线程上下文，如果在中断里休眠，整个 CPU 核心就跟着睡死了。

因此，在 OS 开发中，我们使用的是 自旋锁（Spinlock）。获取不到锁时，它就原地死循环（while !locked {}）。

为了安全地在中断和主程序之间使用自旋锁，OS 理论给出了一个铁律（Locking Rule）：

“当一段普通代码试图获取一个【可能在中断处理程序中也被使用】的锁时，必须先在硬件层面彻底关闭中断！”

回想一下你在上一个实验中学习的 without_interrupts(|| { ... }) 闭包。这就是为什么你在修改时钟计数器、向 APIC 发送 ack() 时，必须把它包裹在关中断的环境中。

主程序调用 without_interrupts 关闭中断（清零 IF 标志）。

主程序获取自旋锁。

执行操作（由于硬件中断被屏蔽，绝对不可能出现中断跑出来抢锁的情况）。

释放自旋锁。

恢复中断标志。

=== 输入缓冲区在什么情况下会满？如果缓冲区满了，用户输入的数据会发生什么情况？

答： 

一、 输入缓冲区在什么情况下会满？

缓冲区的本质是用来“吸收速度差”的。当生产者的生产速度 > 消费者的消费速度，并且持续一段时间后，缓冲区就会满。具体到你的操作系统中，通常有以下几种场景：

1. 突发的大量输入 (Burst Input)

人类打字的速度再快，通常也无法在消费者读取前打满 128 个字符。但如果用户在主机终端上复制并粘贴了一大段文本，或者使用脚本向虚拟机的串口狂发数据。此时，硬件中断会像暴风雨一样密集触发，Top Half 瞬间向队列里塞入几百个字符，瞬间撑爆 128 的容量。

2. 消费者“罢工”或被阻塞 (Consumer Blocked)

如果你的内核主逻辑在忙其他事情，根本没空去调用 pop_key() 读取数据。例如：

你的操作系统正在执行一个极其复杂的数学计算（耗时好几秒）。

系统发生了死锁，或者进入了一个没有调用 get_line() 的死循环。
在这期间，用户哪怕只是慢条斯理地敲了 129 下键盘，缓冲区也会因为“只进不出”而被填满。

3. 频繁且长时间的“关中断”操作

如果你的内核中有很多包裹在 without_interrupts(|| { ... }) 中的长耗时操作，这会阻止调度器切换到负责读取输入的进程，导致输入数据在缓冲区中不断积压。

二、 如果缓冲区满了，用户输入的数据会怎样？

答案是非常残酷但又符合系统设计的：数据会永久丢失（Data Dropped）。

丢失分为两个层面：

1. 软件层面的主动丢弃 (Software Drop)

回顾一下 input.rs 中的这行核心代码：

```rs
if INPUT_BUF.push(key).is_err() {
    warn!("Input buffer is full. Dropping key '{:?}'", key);
}
```

ArrayQueue 是一个固定大小的队列。当里面已经有 128 个未读字符时，第 129 个字符尝试 push 时会返回 Err。
此时，你的代码逻辑捕获了这个错误，打印了一条警告日志，然后什么也没做就结束了函数。这代表着这个字符被内核无情地抛弃了。在用户的视角里，就是“我明明按了键盘，但屏幕上死活不出现那个字母”（即键盘“丢键”）。

2. 硬件层面的溢出错误 (Hardware Overrun Error)

更极端的情况是，如果系统不仅软件缓冲区满了，甚至连中断都被长时间屏蔽，导致 Top Half 的 receive() 都没机会执行。
UART 16550 串口芯片内部通常只有一个极其微小的 16 字节硬件 FIFO(先进先出队列)。一旦这 16 个字节被填满，而 CPU 还没有把它们读走，当第 17 个字节从线缆里传过来时，硬件芯片会直接丢弃这个新字节，并在寄存器中悄悄标记一个 Overrun Error(溢出错误)。

=== 进行下列尝试，并在报告中保留对应的触发方式及相关代码片段：

==== 尝试用你的方式触发 Triple Fault,开启 intdbg 对应的选项，在 QEMU 中查看调试信息，分析 Triple Fault 的发生过程。

为了触发triple fault,我移除了double fault 和 page faul 处理程序,并在kernel_main里面故意去访问一个不存在的物理地址,触发缺页异常。

在终端运行```bash make intdbg ```之后，终端出现了以下日志信息：

```bash
check_exception old: 0x8 new 0xb
Triple fault
```

证明我成功触发了triple Fault。 

用QEMU调试:

#figure(
    image("img/before_triple_fault.png",width:85%)
)

#figure(
    image("img/dbg_triple_fault.png",width: 85%)
)

易知，程序在运行到```rs core::arch::asm!("int 14") ```这句时崩溃，这时候打开另一个运行```bash make debug ```的终端可见到Triple Fault警告。

分析 Triple Fault 的发生过程：

第一阶段：初始异常的爆发 (First Fault)

触发：内核在运行过程中遭遇了常规的异常事件（在本次实验中，是我们通过 int 14 汇编指令强制触发的 Page Fault 缺页异常）。

机制:CPU 暂停当前执行流,试图去中断描述符表(IDT)中寻找 14 号中断的处理函数(page_fault_handler)。

阻碍：由于我们在 IDT 中故意去除了 Page Fault 的注册信息(或者在真实场景中遭遇了严重的栈溢出/页表损坏),CPU 无法成功调用该处理程序。

第二阶段：防线退守与二次沦陷 (Double Fault)

触发：按照 x86 硬件规范,如果在调用异常处理程序的过程中本身又发生了异常,CPU 会立即放弃当前的救救行动，转而呼叫底线救援机制——触发 8 号 Double Fault(双重异常)。

机制:CPU 试图将引发危机的状态压入栈中，并去 IDT 寻找 8 号中断的处理函数(double_fault_handler)。

绝境：在本次实验中，我们同样拆除了 Double Fault 的 IDT 表项。CPU 发现连这道“最后的防线”也是无效的。

第三阶段：三重异常与硬件重置 (Triple Fault & Hardware Reset)

触发：在试图处理 Double Fault 时，再次遭遇了无法解决的异常（如表项缺失）。

结局:面对“处理异常的异常处理过程也发生了异常”这种逻辑死局,CPU 内部硬件逻辑判定系统状态已处于彻底不可知的混沌之中，软件层面失去任何恢复可能。

拉闸:为了保护硬件,CPU 立即停止取指执行，并向主板总线发出硬件重置信号（拉低 RESET 引脚）。在物理机上表现为电脑瞬间黑屏重启；在 QEMU 模拟器中，表现为触发 CPU Reset 并退出进程。

==== 尝试触发 Double Fault,观察 Double Fault 的发生过程，尝试通过调试器定位 Double Fault 发生时使用的栈是否符合预期。

把page fault的处理函数注释掉,再试图访问一个超过范围的页面,就会触发doule fault.

用调试器定位发生double fault的语句:

#figure(
    image("img/double_fault_B1.png",width: 95%)
)
#figure(
    image("img/double_fault_B2.png",width:95%)
)
程序在运行到触发缺页异常的语句的时候，由于找不到缺页异常处理函数，触发了double fault.

在另一个终端中可见到double fault处理函数的输出：
#figure(
    image("img/double_fault_A.png",width: 96%)
)
在调试器中可以查看异常发生前后的栈指针大小：
#figure(
    image("img/double_fault_stack.png",width: 96%)
)
该结果表明，我的栈切换完全正确，且符合预期。

==== 通过访问非法地址触发 Page Fault，观察 Page Fault 的发生过程。分析 Cr2 寄存器的值，并尝试回答为什么 Page Fault 属于可恢复的异常。

缺页异常发生前后的寄存器值：

```bash
Breakpoint 1, 0xffffff0000003000 in ysos_kernel::kernel_main ()
(gdb) monitor info registers

CPU#0
RAX=ffffff00000031b0 RBX=0000000000200000 RCX=0000000005e93ea0 RDX=ffffff01001ffff8
RSI=ffff800000000000 RDI=0000000005e93ea0 RBP=ffffff01001fffe8 RSP=ffffff01001fffe0
R8 =0000000000000000 R9 =0000000000000501 R10=00000000056e6880 R11=0000000000000868
R12=00000000040ad040 R13=ffffffff1ffaffc0 R14=0000000005e96798 R15=00000000055ec018
RIP=ffffff0000003000 RFL=00000082 [--S----] CPL=0 II=0 A20=1 SMM=0 HLT=0
ES =0030 0000000000000000 ffffffff 00cf9300 DPL=0 DS   [-WA]
CS =0038 0000000000000000 ffffffff 00af9a00 DPL=0 CS64 [-R-]
SS =0030 0000000000000000 ffffffff 00cf9300 DPL=0 DS   [-WA]
DS =0030 0000000000000000 ffffffff 00cf9300 DPL=0 DS   [-WA]
FS =0030 0000000000000000 ffffffff 00cf9300 DPL=0 DS   [-WA]
GS =0030 0000000000000000 ffffffff 00cf9300 DPL=0 DS   [-WA]
LDT=0000 0000000000000000 0000ffff 00008200 DPL=0 LDT
TR =0000 0000000000000000 0000ffff 00008b00 DPL=0 TSS64-busy
GDT=     00000000055dc000 00000047
IDT=     0000000005138018 00000fff
CR0=80010033 CR2=0000000000000000 CR3=0000000005801000 CR4=00000668
DR0=0000000000000000 DR1=0000000000000000 DR2=0000000000000000 DR3=0000000000000000 
DR6=00000000ffff0ff0 DR7=0000000000000400
EFER=0000000000000d00
FCW=037f FSW=0000 [ST=0] FTW=00 MXCSR=00001f80
FPR0=0000000000000000 0000 FPR1=0000000000000000 0000
FPR2=0000000000000000 0000 FPR3=0000000000000000 0000
FPR4=0000000000000000 0000 FPR5=0000000000000000 0000
FPR6=0000000000000000 0000 FPR7=8000000000000000 4006
XMM00=0000000000000000 0000000000000000 XMM01=0000000000000000 0000000000000000
XMM02=0000000000000000 0000000000000000 XMM03=0000000000000000 0000000000000000
XMM04=0000000000000000 0000000000000000 XMM05=0000000000000000 0000000000000000
XMM06=0000000000000000 0000000000000000 XMM07=0000000000000000 0000000000000000
XMM08=0000000000000000 0000000000000000 XMM09=0000000000000000 0000000000000000
XMM10=0000000000000000 0000000000000000 XMM11=0000000000000000 0000000000000000
XMM12=0000000000000000 0000000000000000 XMM13=0000000000000000 0000000000000000
XMM14=0000000000000000 0000000000000000 XMM15=0000000000000000 0000000000000000
```
```bash
(gdb) n
Single stepping until exit from function _RNvCs6uHWCmPRqIz_11ysos_kernel11kernel_main,
which has no line number information.
0xffffff0000006e40 in ysos_kernel::interrupt::exceptions::page_fault_handler ()
(gdb) n
Single stepping until exit from function _RNvNtNtCsi7ZoszOsIzC_11ysos_kernel9interrupt10exceptions18page_fault_handler,
which has no line number information.
0xffffff000000e320 in core::panicking::panic_fmt ()
(gdb) monitor info registers

CPU#0
RAX=ffffff00000073a0 RBX=ffffff00000009b1 RCX=0000123456789000 RDX=ffffff0000010458
RSI=ffffff00008162a0 RDI=ffffff0000001385 RBP=ffffff0000816320 RSP=ffffff0000816290
R8 =00000000000003fd R9 =000000000000000d R10=ffffff0000012000 R11=fffffffffffffff8
R12=00000000040ad040 R13=ffffffff1ffaffc0 R14=ffffff0000817388 R15=00000000055ec018
RIP=ffffff000000e320 RFL=00000046 [---Z-P-] CPL=0 II=0 A20=1 SMM=0 HLT=0
ES =0000 0000000000000000 00000000 00000000
CS =0008 0000000000000000 ffffffff 00af9b00 DPL=0 CS64 [-RA]
SS =0000 0000000000000000 00000000 00000000
DS =0010 0000000000000000 ffffffff 00cf9300 DPL=0 DS   [-WA]
FS =0000 0000000000000000 00000000 00000000
GS =0000 0000000000000000 00000000 00000000
LDT=0000 0000000000000000 0000ffff 00008200 DPL=0 LDT
TR =0018 ffffff00008132a0 00000067 00008900 DPL=0 TSS64-avl
GDT=     ffffff0000813310 00000027
IDT=     ffffff0000812290 00000fff
CR0=80010033 CR2=0000123456789000 CR3=0000000005801000 CR4=00000668
DR0=0000000000000000 DR1=0000000000000000 DR2=0000000000000000 DR3=0000000000000000 
DR6=00000000ffff0ff0 DR7=0000000000000400
EFER=0000000000000d00
FCW=037f FSW=0000 [ST=0] FTW=00 MXCSR=00001f80
FPR0=0000000000000000 0000 FPR1=0000000000000000 0000
FPR2=0000000000000000 0000 FPR3=0000000000000000 0000
FPR4=0000000000000000 0000 FPR5=0000000000000000 0000
FPR6=0000000000000000 0000 FPR7=8000000000000000 4006
XMM00=0000000000000000 0000000000000000 XMM01=0000000000000000 0000000000000000
XMM02=0000000000000000 0000000000000000 XMM03=0000000000000000 0000000000000000
XMM04=0000000000000000 0000000000000000 XMM05=0000000000000000 0000000000000000
XMM06=0000000000000000 0000000000000000 XMM07=0000000000000000 0000000000000000
XMM08=0000000000000000 0000000000000000 XMM09=0000000000000000 0000000000000000
XMM10=0000000000000000 0000000000000000 XMM11=0000000000000000 0000000000000000
XMM12=0000000000000000 0000000000000000 XMM13=0000000000000000 0000000000000000
XMM14=0000000000000000 0000000000000000 XMM15=0000000000000000 0000000000000000
(gdb) 
```
📊 寄存器前后对比分析

1. 核心控制寄存器：CR2（决定性的证据）

异常前： CR2=0000000000000000

异常后： CR2=0000123456789000

分析： 这是最关键的变化。当 read_volatile 尝试访问 0x123456789000 时，硬件立刻将其填入 CR2。它直接指证了是哪个虚拟地址导致了“地图（页表）”查询失败。

2. 指令指针与栈指针：RIP & RSP

RIP（执行到哪了）：

前：ffffff0000003000（kernel_main 入口）。

后：ffffff000000e320（已经跳进了 panic_fmt）。

RSP（栈在哪里）：

前：ffffff01001fffe0（初始内核栈）。

后：ffffff0000816290（注意变化！ 地址从 01 开头变成了 00 开头）。

分析： 由于 register_idt 可能为异常配置了 IST（或者跳转到了特定的异常处理栈），RSP 发生了跳变，确保异常处理程序在一个干净、安全的栈上运行。

3. 段寄存器与状态：TR & GDT
异常前： TR=0000，GDT=00000000055dc000（地址在低半区，这是 Bootloader 留下的旧环境）。

异常后： TR=0018，GDT=ffffff0000813310（地址在高半区，且 TR 加载了 0x18 偏移量）。

分析： 这说明在两次快照之间，你的 ysos::init() 成功运行了，它切换了全局描述符表并加载了任务状态段（TSS）。

🛠️ 为什么缺页异常（Page Fault）是可恢复的？

在 x86 架构中，异常被分为三类：Fault（错误）、Trap（陷阱） 和 Abort（中止）。缺页异常属于 Fault，它的设计初衷就是为了“挽救”。

1. 硬件设计的“后悔药”机制

这是最核心的一点：当 Page Fault 发生时，压入栈中的返回地址（RIP）指向的是触发异常的那条指令本身，而不是下一条。

逻辑： 如果操作系统能够修复页表（比如从硬盘加载数据到内存），执行 IRETQ 返回后，CPU 会重新执行刚才失败的那条指令。只要页表修好了，第二次执行就会成功，程序完全察觉不到自己曾经“摔倒”过。

2. 它是虚拟内存管理的基础

如果 Page Fault 不可恢复，现代计算机将无法运行：

按需分页 (Demand Paging)：为了省内存，系统只在程序真正访问某页时才分配物理内存。

交换空间 (Swap)：内存不够时把数据挪到硬盘，访问时再通过补回映射来恢复执行。

共享内存 (CoW)：两个进程共享同一块只读内存，只有当有人尝试写入并触发异常时，才分配新页并恢复运行。

3. 为什么你的程序“死”了？

虽然硬件支持恢复，但软件决定了命运。
代码里，page_fault_handler 最终调用了 panic!。这意味着你的内核告诉 CPU：“我不知道怎么修复这个地址，直接判死刑吧。”于是它就变成了一个不可恢复的错误。

=== 如果在 TSS 中为中断分配的栈空间不足，会发生什么情况？请分析 CPU 异常的发生过程，并尝试回答什么时候会发生 Triple Fault。

答：

1. 栈空间不足引发的连锁反应

假设你为 Double Fault 分配了 4KB 的 IST 栈，但你的处理程序（Handler）逻辑复杂，消耗了超过 4KB 的空间：

第一阶段：静默破坏（Stack Overflow）

当 CPU 执行到 Handler 内部时，随着函数调用加深，RSP 会不断减小，最终越过你分配的 STACK 静态数组边界。由于内核空间通常是连续的，溢出的栈会覆盖掉相邻的内核数据或代码。

第二阶段：触发第二次异常（Secondary Exception）

当溢出的 RSP 最终指向了一个未映射的内存区域，或者试图向只读代码段压栈时，CPU 会立即触发一个新的异常（通常是 Page Fault 或 General Protection Fault）。

第三阶段：异常升级（Exception Promotion）

此时 CPU 陷入了绝境：它正在处理一个异常，结果在处理过程中又产生了一个新的异常。

根据 Intel 手册，如果在处理异常（如 Double Fault）时又发生了异常，且该异常无法被嵌套处理，CPU 会判定为 Triple Fault。

2. 什么时候会发生 Triple Fault？

Triple Fault (三重故障) 是 CPU 的“终极放弃”。当 CPU 无法调用异常处理程序来处理当前的异常时，它就会停止工作，并向系统发送一个信号，通常表现为虚拟机瞬间重启。

发生 Triple Fault 的典型场景包括：

A. 栈切换失败（最常见原因）

这是你目前正在攻克的难关。当异常发生，CPU 尝试通过 TSS 切换栈时，如果发生以下情况，会直接触发 Triple Fault：

无效地址：TSS 中填写的 IST 地址是一个未在页表中映射的虚空地址（如你之前遇到的高位补全失败）。

权限错误：IST 地址指向的页面标记为“只读”或“不存在”。

递归崩溃：CPU 尝试向 IST 栈压入现场，但该操作本身触发了 Page Fault，而 CPU 此时已经认为自己在处理双重错误。

B. IDT 配置错误

如果异常发生时，CPU 试图去查 IDT 表，但：

IDT 越界：异常向量号超过了 IDTR.limit。

描述符无效：IDT 里的 Gate 描述符被设为了 Present = 0。

CS 错误：IDT 描述符里填写的代码段选择子在 GDT 中不存在。

C. 递归异常无法处理

如果发生了一个异常（如 Page Fault），CPU 试图调用 Handler，但 Handler 没注册。于是升级为 Double Fault。如果 Double Fault 也没注册或者其 Handler 本身又崩了，CPU 就彻底死心，进入 Triple Fault 关机重启。

3. CPU 异常发生的底层路径图

我们可以把这个过程看作是一个三级跳：

一级异常 (First-level Exception)：

例如：你代码里的 read_volatile(0xdeadbeef) 触发  Page Fault。

CPU 尝试压栈保存现场。

二级异常 (Double Fault)：

如果压栈失败，或者 IDT 里没对应的门。

CPU 查找 IDT 第 8 号向量。

关键点：如果 double fault 设置了 IST，CPU 强行跳到新栈。

三级异常 (Triple Fault)：

如果 CPU 在尝试进入 double fault Handler 的过程中再次失败（如：TSS 基址非法、IST 栈指针所在的页表没映射）。

结局：CPU 停机（Halt），并触发硬件重置信号。

💡 总结与建议

栈空间不足：不会立即重启，但会破坏周围数据，最终因访问非法内存而导致系统崩溃。

Triple Fault：通常发生在异常跳转的过渡瞬间。如果你看到 QEMU 瞬间重启，90% 的概率是你的 TSS 地址、GDT 权限或者页表映射（对应栈空间）没写对。

=== 在未使用 set_stack_index 函数时，中断处理程序的栈可能哪里？尝试结合 gdb 调试器，找到中断处理程序的栈，并验证你的猜想是否正确。

答：如果没有使用这个函数的话，中断处理程序可能会原地压栈。我把缺页异常处理的set_stack_index函数注释掉了，然后触发缺页异常之后查看栈指针，结果：

#figure(
    image("img/page_fault_not_set.png",width: 100%)
)

📊 实验数据分析

当前 RSP 值：0xffffff01001fff20

对比之前的 Double Fault 实验：

在之前的实验中，触发 Double Fault 且开启了 IST 切换后，RSP 跳到了 0xffffff0000813048（那个专门分配的高半区 TSS 栈）。

而在注释掉 set_stack_index 后的这次实验，RSP 依然停留在 0xffffff01... 这个地址段。

🕵️ 结论：猜想验证成功

没有发生栈切换：

此时的 RSP 指针（01 开头）与触发异常前的内核主栈是在同一个内存区域的。这证明：如果没有显式指定 IST 索引，CPU 默认会直接使用发生中断那一刻的栈指针。

原地压栈的行为：

当 Page Fault 发生时，硬件直接在当前的内核栈（Ring 0 栈）上压入了 SS, RSP, RFLAGS, CS, RIP 以及 Error Code。

⚠️ 这种做法的“危险性”

既然能跑通，为什么我们还要大费周章地写那个函数？

现在的 page_fault_handler 正在使用主内核栈。如果是因为栈溢出（Stack Overflow）导致的缺页，那么：

当前的 RSP 已经越界，指向了一个非法区域。

CPU 尝试在“坏掉”的 RSP 处压入现场。

二次崩溃：压栈操作本身触发了第二次 Page Fault。

由于没有独立的 IST 逃生通道，CPU 无法处理这种递归异常，会直接导致 Triple Fault（虚拟机瞬间重启）。

💡 总结

实验清楚地展示了：

不调用该函数：中断处理程序与被中断的代码共用同一个栈，这在正常情况下没问题，但在处理严重错误（如 Double Fault）时极度危险。

调用该函数：中断处理程序会强制切换到独立、安全的 IST 栈，确保即便主栈烂透了，内核也能稳稳地接住异常并输出调试信息。
