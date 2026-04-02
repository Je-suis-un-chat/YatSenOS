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

值得注意的是，我的整个实验文件夹是YatSenOS,具体的实验代码在yatsenos文件夹下，一开始我是在YatSenOS中进行```bashcp```命令的，导致文件结构完全乱套。于是我用```bashrm```命令将拷错的文件删掉，再在子目录中把官方代码拉进来。

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

        // FIXME: fill tss.interrupt_stack_table with the static stack buffers like above
        // You can use `tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize]`
        // 2. 补全 Interrupt Stack Table (IST)
        
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
