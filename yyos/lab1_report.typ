#import "../template/report.typ": *
#show raw.where(block: true): set block(breakable: true)


#show: report.with(
  title: "操作系统实验报告",
  subtitle: "实验一：操作系统的启动",
  name: "郭盈盈",
  stdid: "24312063",
  classid: "吴岸聪老师班",
  major: "保密管理",
  school: "计算机学院",
  time: "2025 学年第二学期",
  banner: "./images/sysu.png"
)

= 实验目的

1. 了解页表的作用、ELF 文件格式、操作系统在 x86 架构的基本启动过程。
2. 尝试使用 UEFI 加载并跳转到内核执行内核代码。
3. 实现基于 uart16550 的串口驱动，使用宏启用输出能力、并启用日志系统。
4. 学习并尝试使用调试器对内核进行调试。

= 实验内容

1. 编译内核ELF
2. 在UEFI中加载内核：
- 加载相关文件
- 更新控制寄存器
- 映射内核文件
- 跳转执行
- 调试内核
3.UART与日志输出
- 串口驱动
- 日志输出
- Panic处理
4.思考题

= 实验过程
== 编译内核ELF
=== 步骤一：知识学习
==== 了解分页内存
页表是操作系统中常见的内存模型，它是一个虚拟地址到物理地址的映射表，通过它可以实现虚拟内存。

在x64架构中，页表不再是一个简单连续的映射表，而是一个树状结构，这样的结构被称为多级页表。多级页表的设计使得内存管理更加灵活和高效，可以支持更大的地址空间。在本实验的实现中，会使用到四级页表。

页表分级的设计目的和映射方式：
- 一级页表（PML4）：包含512个条目，每个条目指向一个二级页表。
- 二级页表（PDPT）：包含512个条目，每个条目指向一个三级页表。
- 三级页表（PD）：包含512个条目，每个条目指向一个四级页表。
- 四级页表（PT）：包含512个条目，每个条目指向一个物理页框。
每个页表条目包含了物理地址和一些标志位，如存在位、读写位、用户/内核位等。通过设置这些标志位，操作系统可以控制内存访问权限和行为。   

本实验直接使用 x86_64crate 提供的PageTable 结构体和 Cr3 寄存器的封装,来实现页表的创建和管理。

这个封装提供了：
1. 类型安全的页表： 提供 PageTable 结构体，你可以像操作数组一样访问那 512 个项，而不是直接操作内存地址。

2.  映射器 (Mapper)： 提供 OffsetPageTable 或 MappedPageTable 等抽象。你只需要调用 .map_to() 函数，它会自动帮你完成 L4 -> L3 -> L2 -> L1 的逐级查找和填充。

3. 寄存器包装： 提供 Cr3::write() 这种函数。你传给它一个物理地址，它帮你执行那条把地址塞进 CPU 核心的指令。

4. 位标志定义： 提供 PageTableFlags，让你用 WRITABLE | PRESENT 这种人类能读懂的单词，而不是 0x3 这种神秘数字。

==== 了解ELF文件格式

ELF（Executable and Linkable Format）是一种常见的可执行文件格式，广泛用于类Unix系统中。它定义了文件的结构和内容，包括代码段、数据段、符号表等信息。在操作系统开发中，内核通常以ELF格式编译生成，这样可以方便地加载和执行。

ELF文件大体上由文件头和数据组成，它还可以加上额外的调试信息。

一般来说，ELF文件包含以下几个重要部分：
- ELF Header：包含文件的基本信息，如类型、架构、入口点地址等。
- Program Header Table：描述了程序的内存映射，包括代码段、数据段等。
- Section Header Table：描述了文件的各个节（section），如符号表、字符串表等。
- Code and Data Sections：包含实际的代码和数据。
在本实验中，我们需要编译内核代码生成一个ELF文件，然后在UEFI环境中加载这个ELF文件并执行其中的代码。

控制ELF的结构：

- 源码层：使用编译器属性

```rs
int __attribute__((section(".myvariable"))) a = 0;
int __attribute__((section(".myfunction"))) main() { return 0; }
```

效果:强制将特定的数据或代码放入自定义的section中，而不是默认的.text或.data等section。这对于操作系统开发非常有用，可以让我们更精确地控制内核的内存布局。

- 链接器层：使用链接器脚本
链接器脚本是一种文本文件，定义了如何将不同的输入文件和section映射到最终的内存地址空间中。通过链接器脚本，我们可以指定特定的section应该被放置在内核映像的哪个位置，以及它们的对齐方式等属性。

```rs
SECTIONS
{
    . = 0xc0ffee00;
    .text : { *(.text) }
    . = ALIGN(0x1000);
    .data : { *(.data) }
    .bss : { *(.bss) }
}
```
效果:将.text段放在0xc0ffee00地址开始，.data段紧跟其后，并且.bss段对齐到0x1000边界。这种方式可以让我们精确控制内核的内存布局，确保代码和数据被放置在预期的位置。

- 编译指令：控制调试与符号信息

```bash
gcc main.c -c -o main.o && ld main.o -T ./script.ld -o main
```
效果:编译生成一个ELF文件，并且通过链接器脚本控制其内存布局。这个命令会将main.c编译成main.o，然后使用链接器脚本将main.o链接成一个可执行的ELF文件main。

参数解释：
```-c```：告诉编译器只编译源文件，不进行链接，生成一个目标文件（.o）。
```-o main.o```：指定输出文件名为main.o。
```-T ./script.ld```：指定链接器脚本文件为script.ld，这个脚本定义了ELF文件的内存布局和section的映射。
```-o main```：指定最终生成的可执行文件名为main。

为什么需要控制ELF的结构？

在操作系统开发中，内核需要被加载到特定的内存地址，并且需要有特定的布局来满足硬件和软件的要求。通过控制ELF文件的结构，我们可以确保内核代码和数据被放置在正确的位置，从而保证内核能够正确地运行。

1. 适配硬件与内存映射

只有将权限相同的section打包进同一个segment，才能正确设置段权限（如可执行、可写等），否则可能会导致内核无法正常运行。

2. 定义程序“生存空间”

- 内核开发：内核通常需要运行在高位虚拟地址，必须通过链接脚本强制指定。

- 裸机/嵌入式：硬件寄存器或存储器的物理地址是固定的，必须通过ELF结构确保代码被加载到正确位置。

3. 优化加载效率

- 合并加载：OS加载器以segment为单位进行搬运。通过控制ELF结构，减少segment的数量可以显著加快程序启动速度。

- 按需加载：只有标记为LOAD 类型的段才会被分配物理内存，其余部分留在磁盘上，节省内存空间。

4. 安全加固

通过控制ELF结构，可以将敏感代码和数据放在特定的section中，并设置适当的权限（如只读、不可执行等），从而提高系统的安全性。

=== 步骤二：编译内核ELF
按照实验文档，我在 crates/kernel 目录下运行了 ```rt cargo build --release ```,但是出现了几个问题导致编译失败，我排查了这些报错并逐一解决。最后完成了这部分任务。
==== 问题一：找不到全局内存分配器
在普通应用程序里，如果想用动态内存，底层会自动调用操作系统的 ```rt malloc ```和```rt free```。但是，由于我正在写操作系统内核本身，(```rt #![no_std]```环境)，我的代码底下并没有另一个操作系统来提供```rt malloc```。当我引入了相关核心库时，Rust编译器就会要求指定分配内存的负责人。如果没有，编译器就会拒绝编译。

于是，就有如下报错信息：
```bash
error: no global memory allocator found but one is required; link to std or add
 #[global_allocator] to a static item that implements the GlobalAlloc trait
```

为了解决这个问题，我在 kernel/src/main.rs 文件最下方添加了代码：

```rs
struct DummyAllocator;

unsafe impl core::alloc::GlobalAlloc for DummyAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

#[global_allocator]
static ALLOCATOR: DummyAllocator = DummyAllocator;
```
这段代码给程序添加了一个名为```rt DummyAllocator```的伪分配器，满足了编译条件。

==== 问题二：命令参数缺失
在运行这个命令时，报了如下错误：
#figure(
  image("./img/error2.png", width: 70%),
)
按照提示，我直接在命令后面添加相应参数即可解决：

```bash
cargo +nightly build --release -Z build-std -Z json-target-spec
```

==== 问题三：Cargo 依赖下载网络失败
在拉取 crates.io 索引时，频繁报出如下错误信息：

```bash
(base) je-suis-un-chat@LAPTOP-MAGCR3QA:~/YatSenOS/lab1/crates/kernel$ cargo build --release -Z json-target-spec
   Updating `rsproxy-sparse` index
   Locking 30 packages to latest Rust 1.96.0-nightly compatible versions
warning: spurious network error (2 tries remaining): [35] SSL connect error (TLS connect error: error:0A000126:SSL routines::unexpected eof while reading)
etwork error (2 tries remaining): [35] SSL connect error (TLS connect error: error:0A000126:SSL routines::unexpected eof while reading)
error: failed to download from `https://rsproxy.cn/api/v1/crates/x86/0.52.0/download`
Caused by:warning: spurious network error (2 tries remaining): [35] SSL connect error (TLS connect error: error:0A000126:SSL routines::unexpected eof while reading)
warning: spurious network error (2 tries remaining): [35] SSL connect error (TLS connect error: error:0A000126:SSL routines::unexpected eof while reading)
warning: spurious network error (2 tries remaining): [35] SSL connect error (TLS connect error: error:0A000126:SSL routines::unexpected eof while reading)
warning: spurious n
  [35] SSL connect error (TLS connect error: error:0A000126:SSL routines::unexpected eof while reading)
```
为了解决这个问题，我在 config.toml 文件下添加了清华大学的镜像源：

```toml
[source.crates-io]
replace-with = 'tuna'

[source.tuna]
registry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"

[net]
git-fetch-with-cli = true
```
但是还是出现了如下报错：

```bash
(base) je-suis-un-chat@LAPTOP-MAGCR3QA:~/YatSenOS/lab1/crates/kernel$ cargo update
Updating `tuna` index
warning: spurious network error (3 tries remaining): [35] SSL connect error (TLS connect error: error:0A000126:SSL routines::unexpected eof while reading); class=Net (12)
warning: spurious network error (2 tries remaining): [35] SSL connect error (TLS connect error: error:0A000126:SSL routines::unexpected eof while reading); class=Net (12)
warning: spurious network error (1 try remaining): [35] SSL connect error (TLS connect error: error:0A000126:SSL routines::unexpected eof while reading); class=Net (12)
error: failed to get `arrayvec` as a dependency of package `ysos_boot v0.1.0 (/home/je-suis-un-chat/YatSenOS/lab1/crates/boot)`

Caused by:
failed to load source for dependency `arrayvec`

Caused by:
unable to update registry `crates-io`

Caused by:
failed to update replaced source registry `crates-io`

Caused by:
failed to fetch `https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git`

Caused by:
network failure seems to have happened
if a proxy or similar is necessary `net.git-fetch-with-cli` may help here
https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli

Caused by:
[35] SSL connect error (Send failure: Broken pipe); class=Net (12)
(base) je-suis-un-chat@LAPTOP-MAGCR3QA:~/YatSenOS/lab1/crates/kernel$
```

这是因为我使用的清华源还在用旧的 Git 协议，由于网络环境或 SSL 证书问题，握手时发生了 ```bashBroken pipe```。

为了解决这个问题，我在 config.toml 文件中添加了一行配置：

```toml
[net]
git-fetch-with-cli = true
```
改用 Ubuntu 系统自带的 git 命令行工具来解决 SSL 错误。

我还调整了全局 Git 配置，让它能适配我的清华源：

```bash
# 增加 Git 的网络缓冲区大小（防止大文件传输中断）
git config --global http.postBuffer 524288000

# 如果是因为 SSL 证书链验证问题（常见于校园网或公司内网代理），可以临时关闭验证
git config --global http.sslVerify false

# 设置低速连接不超时（防止在索引更新慢时断开）
git config --global http.lowSpeedLimit 0
git config --global http.lowSpeedTime 999999
```

在再次执行命令前，彻底清空代理：

```bash 
unset http_proxy
unset https_proxy
unset ALL_PROXY
cargo update
```
最终解决了这个网络问题。

解决了上述三个问题后，我的 ELF 内核终于编译成功了。

=== 步骤三：找到编译产物并用命令查看其基本信息

使用 ```bashreadelf```命令查看编译产物基本信息：

#figure(
  image("img/readelf_l.png", width: 120%),
)
#figure(
  image("img/readelf_h.png", width: 120%),
)
#figure(
  image("img/readelf_S.png", width: 120%),
)

=== 步骤四：回答实验任务问题

==== Q1：编译产物的架构相关信息，与配置文件中的描述是否一致？
A1：编译产物的各项底层架构参数与 x86_64-unknown-none.json 配置文件中的描述完美吻合。这说明 Rust 编译器（rustc）成功读取了我们提供的自定义目标配置（Target JSON），并严格按照该配置的要求，完成了跨平台交叉编译（Cross-compilation）工作，生成了符合 YatSenOS 要求的裸机（none 操作系统）二进制执行文件。

==== Q2：找出内核的入口点，它是被如何控制的？结合源码、链接、加载的过程，谈谈你的理解。
A2：在计算机体系结构和操作系统工程中，内核入口点的控制机制是一个跨越编译期、链接期和运行期的严格协同过程。内核的入口点并不是硬件自动识别的，而是通过源码定义、链接器硬编码、加载器解析跳转这三个阶段来实现控制权的精确转移。

在源码层面，内核通过调用底层的宏（```rt boot::entry_point! ```),在语言级别静态定义了一个不受编译器名称修饰机制干扰的全局符号(```rt _start ```),从而向外部提供了一个具有标准调用约定的确定性标识。

进入链接阶段后，链接器严格遵循自定义链接脚本(```rt kernel.ld```)的内存布局规范，通过```rt ENTRY ```指令声明将该符号解析为程序的唯一入口，并根据位置计数器为其所在的代码段分配绝对的内存基址，最终将该地址硬编码写入最终生成的ELF可执行文件头部的入口字段中。

在系统启动的加载阶段，先期运行的引导程序作为加载器，会动态解析该ELF文件的文件头和程序头表，按规范将内核的代码段和数据段等搬运至物理内存的对应地址空间，随后直接提取出头部预存的入口地址数值，将其强制转换为底层的无返回值函数指针，最后通过执行底层的机器级跳转指令直接修改中央处理器的指令指针寄存器，完成从引导环境到内核初始指令执行流的精确控制权转移。

==== Q3：请找出编译产物的 segments 的数量，并且用表格的形式说明每一个 segments 的权限、是否对齐等信息。

#figure(
  caption: [编译产物 Segments 权限与对齐信息表],
  table(
    columns: 6,
    align: center + horizon,
    
    // 表头
    [*序号*], [*类型 (Type)*], [*权限标志 (Flg)*], [*实际权限*], [*对齐 (Align)*], [*映射的节 (Section)*],
    
    // 数据行
    [00], [LOAD], [R E], [可读、可执行], [`0x1000`], [`.text`],
    [01], [LOAD], [R],   [只读],         [`0x1000`], [`.rodata`],
    [02], [LOAD], [RW],  [可读、可写],   [`0x1000`], [`.bss`],
    [03], [GNU_STACK], [RW], [可读、可写], [`0x0`],    [（无）],
  )
)

== 在UEFI中加载内核
=== 步骤一：阅读并理解 crates/boot 文件夹中的代码文件
==== boot 文件夹的总体作用
在CPU上电后，它并不知道如何运行 Rust 编写的内核。boot 文件夹的作用就是：

1. 接管硬件：通过UEFI固件获得访问磁盘、显示器和内存的权限。

2. 环境初始化：开启分页机制，建立虚拟地址映射。

3. 搬运内核：从磁盘找到内核ELF文件，把它加载到物理内存中。

4. 权力交接：把收集到的硬件信息打包成 BootInfo ，然后跳进内核入口点。

==== config.rs 文件
读取并解析文本形式的配置文件。

==== fs.rs 文件
通过UEFI协议与磁盘打交道，负责把内核文件从磁盘搬到内存里。

==== allocator.rs 文件
提供引导阶段的动态内存管理。

==== lib.rs 文件
确保内核知道从哪里开始运行，以及如何拿到引导程序准备好的数据或程序。

==== main.rs 文件
控制整个引导流程的生命周期。

=== 步骤二：补全 boot/src/main.rs 代码

==== 加载相关文件

在 crates/boot/src/fs.rs 中，提供了一些文件操作相关的函数可调用，使用这些函数分别加载配置文件和内核 ELF ，代码如下：

```rs
 // 1. 加载并解析配置文件
    let mut config_file = fs::open_file("\\EFI\\BOOT\\boot.conf");
    let config_content = fs::load_file(&mut config_file);
    let config = config::Config::parse(config_content);

    info!("Config解析成功: {:#x?}", config);

    // 2. 加载内核 ELF 文件
    let mut kernel_file = fs::open_file(config.kernel_path);
    let kernel_content = fs::load_file(&mut kernel_file);
    let elf = xmas_elf::ElfFile::new(kernel_content).expect("Failed to parse kernel ELF");

    unsafe {
        set_entry(elf.header.pt2.entry_point() as usize);
    }
```

==== 更新控制寄存器

CR0 寄存器是x86 架构下的一个控制寄存器，它决定了CPU的工作模式和内存管理行为。其中第16位被称为WP位（写保护位）。在正常的保护模式或长模式下，如果一个内存页在页表被标记为“只读”，那么即便是内核代码，一旦尝试写入该区域，CPU硬件就会立刻触发页错误，直接导致系统崩溃或重启。

为了进行后续读取并映射内核ELF文件的操作，现在需要使用Cr0寄存器禁用根页表的写保护，以便进行后续的内存映射操作。

```rs
unsafe {
        // 暂时关闭写保护以修改页表
        Cr0::update(|f| f.remove(Cr0Flags::WRITE_PROTECT));
    }
```
==== 映射内核文件并跳转执行
===== 前置知识：分页内存管理
虽然前文已经写过，为了保持每个子任务的相对独立性和完整性，在这里再简要概述：

1.页表

页表是操作系统中常用的内存模型，用它可实现虚拟内存、进行内存隔离、避免内存碎片等。它是一个虚拟地址到物理地址的映射表，通过它可实现虚拟内存。

2.页表表项

页表中的每一个表项都是一个64位的数据，能够描述一个页表项的属性，及一些权限管理：

R（是否可读）；

W（是否可写）；

X（是否可执行）；

3. 虚拟内存

通过利用页表，操作系统可以实现虚拟内存，将一部分分配出的物理内存映射到虚拟地址空间中。

4. x64 相关寄存器

在 x64 架构中，有一些特殊的寄存器与页表相关，且在本实现中会用到，它们分别是：

- CR0: 控制寄存器，存储了一些控制系统运行状态的标志位，包括根页表的写保护位。

- CR2: 页错误地址寄存器，存储了最近一次页错误的虚拟地址。

- CR3: 页表根地址寄存器，存储了页表的物理地址，也就是PML4 的地址。

===== 前置知识：内核ELF文件格式

ELF是一种文件格式，既可用作可执行文件，也可用作共享文件库文件。ELF文件大体上由文件头和数据组成，还可以加上额外的调试信息。

一般来说，ELF 有以下几个部分：

- ELF文件头

- section header 

- program header 

- 文件内容：section 和 segment

===== 前置知识：操作系统在x86架构的基本启动过程

1. 上电自检：按下电源键，电源管理器向CPU发送 RESET 信号。CPU 初始化寄存器。

2. 固件接管：CPU开始执行主板Flash芯片上的固件代码(BIOS 或 UEFI)

3. 硬件扫描：初始化内存、检查显卡、扫描磁盘总线。

4.UEFI 加载：

- 初始化协议栈：建立起完整的文件系统支持(FAT32)、网络协议栈和图形输出(GOP)

- 加载 EFI 应用程序：固件直接读取文件系统里的 .efi 文件，如果没有注册条目，它会寻找默认的\EFI\BOOT\BOOTX64.EFI

- ExitBootServices:启动程序准备好内存映射后，调用此服务。UEFI固件释放对硬件的控制权，系统正式进入OS掌控时间

5.引导CPU进入64位长模式：开启PAE、加载页表、设置EFER.LME、开启分页

===== 架构了解：阅读配置文件定义中有关内核栈的内容

这份代码为YatSenOS的引导程序定义了一个配置解析器，它通过读取并解析一个名为boot.conf的文本文件，动态地决定内核在启动时的内存布局参数（如内存栈的虚拟基地址、2MB的栈空间大小以及物理内存映射偏移量），从而让开发者无需重新编译代码，就能灵活地控制内核运行时的硬件环境和路径信息。

===== 了解了前置知识和项目架构之后，对代码进行以下补全：
#block(breakable: true, width: 100%)[
```rs

    // 4. 建立虚拟内存映射
    let mut page_table = current_page_table();
    let mut frame_allocator = UEFIFrameAllocator;

    unsafe {
        // 暂时关闭写保护以修改页表
        Cr0::update(|f| f.remove(Cr0Flags::WRITE_PROTECT));
    }

    // --- 步骤 4.1: 线性映射整个物理内存 ---
    // 这是修复 Page Fault 的关键：为 load_elf 提供可写的线性地址空间
    elf::map_physical_memory(
        config.physical_memory_offset,
        max_phys_addr,
        &mut page_table,
        &mut frame_allocator,
    );

  ```
  ```rs

    // --- 步骤 4.2: 加载内核段 ---
    elf::load_elf(
        &elf,
        config.physical_memory_offset, 
        &mut page_table, 
        &mut frame_allocator,
    ).expect("Failed to load and map ELF segments");
    
    // --- 步骤 4.3: 映射内核栈 ---
    elf::map_range(
        config.kernel_stack_address,
        config.kernel_stack_size,
        &mut page_table,
        &mut frame_allocator,
    ).expect("Failed to map kernel stack");

    unsafe {
        // 重新开启写保护
        Cr0::update(|f| f.insert(Cr0Flags::WRITE_PROTECT));
    }

    free_elf(elf);

```]
#figure(
  image("img/yassenos.png", width: 80%),
)
==== 调试内核
- 在第一个终端启动QEMU：

```bash
make debug
```

- 在另一个终端启动GDB:

```bash
gdb target/x86_64-unknown-none/release-with-debug/ysos_kernel
```

-进入GDB的黑框界面后，依次输入以下命令：

```bash
target remote :1234   #连接虚拟机
layout split          #加载源代码界面
b init                #在init函数处打断点
c                     #让程序跑起来
```
调试页面：
#figure(
    image("img/dg.png",width:  67%),
)

用QEMU的monitor模式查看内存布局：

```bash
(qemu) info mem
0000000000000000-0000000004a00000 0000000004a00000 -rw
0000000004a00000-0000000004c00000 0000000000200000 -r-
0000000004c00000-0000000005800000 0000000000c00000 -rw
0000000005800000-0000000005e00000 0000000000600000 -r-
0000000005e00000-0000010000000000 000000fffa200000 -rw
ffff800000000000-ffff810000200000 0000010000200000 -rw
ffffff0000000000-ffffff0000004000 0000000000004000 -r-
ffffff0000004000-ffffff0000007000 0000000000003000 -rw
ffffff0100000000-ffffff0100200000 0000000000200000 -rw
(qemu) info mtree
address-space: i440FX
  0000000000000000-ffffffffffffffff (prio 0, i/o): bus master container
    0000000000000000-ffffffffffffffff (prio 0, i/o): alias bus master @system 0000000000000000-ffffffffffffffff

address-space: I/O
  0000000000000000-000000000000ffff (prio 0, i/o): io
    0000000000000000-0000000000000007 (prio 0, i/o): dma-chan
    0000000000000008-000000000000000f (prio 0, i/o): dma-cont
    0000000000000020-0000000000000021 (prio 0, i/o): pic
    0000000000000040-0000000000000043 (prio 0, i/o): pit
    0000000000000060-0000000000000060 (prio 0, i/o): i8042-data
    0000000000000061-0000000000000061 (prio 0, i/o): pcspk
    0000000000000064-0000000000000064 (prio 0, i/o): i8042-cmd
    0000000000000070-0000000000000071 (prio 0, i/o): rtc
      0000000000000070-0000000000000070 (prio 0, i/o): rtc-index
    000000000000007e-000000000000007f (prio 0, i/o): kvmvapic
    0000000000000080-0000000000000080 (prio 0, i/o): ioport80
    0000000000000081-0000000000000083 (prio 0, i/o): dma-page
    0000000000000087-0000000000000087 (prio 0, i/o): dma-page
    0000000000000089-000000000000008b (prio 0, i/o): dma-page
    000000000000008f-000000000000008f (prio 0, i/o): dma-page
    0000000000000092-0000000000000092 (prio 0, i/o): port92
    00000000000000a0-00000000000000a1 (prio 0, i/o): pic
    00000000000000b2-00000000000000b3 (prio 0, i/o): apm-io
    
    00000000000000c0-00000000000000cf (prio 0, i/o): dma-chan
    00000000000000d0-00000000000000df (prio 0, i/o): dma-cont
    00000000000000f0-00000000000000f0 (prio 0, i/o): ioportF0
    0000000000000170-0000000000000177 (prio 0, i/o): ide
    00000000000001ce-00000000000001d1 (prio 0, i/o): vbe
    00000000000001f0-00000000000001f7 (prio 0, i/o): ide
    0000000000000376-0000000000000376 (prio 0, i/o): ide
    0000000000000378-000000000000037f (prio 0, i/o): parallel
    00000000000003b4-00000000000003b5 (prio 0, i/o): vga
    00000000000003ba-00000000000003ba (prio 0, i/o): vga
    00000000000003c0-00000000000003cf (prio 0, i/o): vga
      ```
```bash
    00000000000003d4-00000000000003d5 (prio 0, i/o): vga
    00000000000003da-00000000000003da (prio 0, i/o): vga
    00000000000003f1-00000000000003f5 (prio 0, i/o): fdc
    00000000000003f6-00000000000003f6 (prio 0, i/o): ide
    00000000000003f7-00000000000003f7 (prio 0, i/o): fdc
    00000000000003f8-00000000000003ff (prio 0, i/o): serial
    00000000000004d0-00000000000004d0 (prio 0, i/o): elcr
    00000000000004d1-00000000000004d1 (prio 0, i/o): elcr
    0000000000000510-0000000000000511 (prio 0, i/o): fwcfg
    0000000000000514-000000000000051b (prio 0, i/o): fwcfg.dma
    0000000000000cf8-0000000000000cfb (prio 0, i/o): pci-conf-idx
    0000000000000cf9-0000000000000cf9 (prio 1, i/o): piix-reset-control
    0000000000000cfc-0000000000000cff (prio 0, i/o): pci-conf-data
    0000000000005658-0000000000005658 (prio 0, i/o): vmport
    000000000000ae00-000000000000ae17 (prio 0, i/o): acpi-pci-hotplug
    000000000000af00-000000000000af0b (prio 0, i/o): acpi-cpu-hotplug
    000000000000afe0-000000000000afe3 (prio 0, i/o): acpi-gpe0
    000000000000b000-000000000000b03f (prio 0, i/o): piix4-pm
      000000000000b000-000000000000b003 (prio 0, i/o): acpi-evt
      000000000000b004-000000000000b005 (prio 0, i/o): acpi-cnt
      000000000000b008-000000000000b00b (prio 0, i/o): acpi-tmr
    000000000000b100-000000000000b13f (prio 0, i/o): pm-smbus
    000000000000c000-000000000000c00f (prio 1, i/o): piix-bmdma-container
      000000000000c000-000000000000c003 (prio 0, i/o): piix-bmdma
      000000000000c004-000000000000c007 (prio 0, i/o): bmdma
      000000000000c008-000000000000c00b (prio 0, i/o): piix-bmdma
      000000000000c00c-000000000000c00f (prio 0, i/o): bmdma

address-space: cpu-smm-0
  0000000000000000-ffffffffffffffff (prio 0, i/o): memory
    0000000000000000-00000000ffffffff (prio 1, i/o): alias smram @smram 0000000000000000-00000000ffffffff
    0000000000000000-ffffffffffffffff (prio 0, i/o): alias memory @system 0000000000000000-ffffffffffffffff

address-space: piix3-ide
  0000000000000000-ffffffffffffffff (prio 0, i/o): bus master container
    0000000000000000-ffffffffffffffff (prio 0, i/o): alias bus master @system 0000000000000000-ffffffffffffffff

address-space: VGA
  0000000000000000-ffffffffffffffff (prio 0, i/o): bus master container
    0000000000000000-ffffffffffffffff (prio 0, i/o): alias bus master @system 0000000000000000-ffffffffffffffff
```
     ```bash
address-space: PIIX3
  0000000000000000-ffffffffffffffff (prio 0, i/o): bus master container
    0000000000000000-ffffffffffffffff (prio 0, i/o): alias bus master @system 0000000000000000-ffffffffffffffff

address-space: PIIX4_PM
  0000000000000000-ffffffffffffffff (prio 0, i/o): bus master container
    0000000000000000-ffffffffffffffff (prio 0, i/o): alias bus master @system 0000000000000000-ffffffffffffffff

address-space: cpu-memory-0
address-space: memory
  0000000000000000-ffffffffffffffff (prio 0, i/o): system
    0000000000000000-0000000005ffffff (prio 0, ram): alias ram-below-4g @pc.ram 0000000000000000-0000000005ffffff
    0000000000000000-ffffffffffffffff (prio -1, i/o): pci
      00000000000a0000-00000000000affff (prio 2, ram): alias vga.chain4 @vga.vram 0000000000000000-000000000000ffff
      00000000000a0000-00000000000bffff (prio 1, i/o): vga-lowmem
      00000000000c0000-00000000000dffff (prio 1, rom): pc.rom
      00000000000e0000-00000000000fffff (prio 1, rom): alias isa-bios @pc.bios 00000000003e0000-00000000003fffff
      0000000080000000-0000000080ffffff (prio 1, ram): vga.vram
      0000000081010000-0000000081010fff (prio 1, i/o): vga.mmio
        0000000081010000-000000008101017f (prio 0, i/o): edid
        0000000081010400-000000008101041f (prio 0, i/o): vga ioports remapped
        0000000081010500-0000000081010515 (prio 0, i/o): bochs dispi interface
        0000000081010600-0000000081010607 (prio 0, i/o): qemu extended regs
      00000000ffc00000-00000000ffffffff (prio 0, rom): pc.bios
    00000000000a0000-00000000000bffff (prio 1, i/o): alias smram-region @pci 00000000000a0000-00000000000bffff
    00000000000c0000-00000000000c3fff (prio 1, ram): alias pam-rom @pc.ram 00000000000c0000-00000000000c3fff
    00000000000c4000-00000000000c7fff (prio 1, i/o): alias pam-pci @pci 00000000000c4000-00000000000c7fff
    00000000000c8000-00000000000cbfff (prio 1, i/o): alias pam-pci @pci 00000000000c8000-00000000000cbfff
    00000000000cc000-00000000000cffff (prio 1, i/o): alias pam-pci @pci 
    : alias pam-pci @pci 
    
    00000000000cc000-00000000000cffff
    00000000000d0000-00000000000d3fff (prio 1, i/o): alias pam-pci @pci 00000000000d0000-00000000000d3fff
    00000000000d4000-00000000000d7fff (prio 1, i/o): alias pam-pci @pci 00000000000d4000-00000000000d7fff
    00000000000d8000-00000000000dbfff (prio 1, i/o)
    00000000000d8000-00000000000dbfff
    00000000000dc000-00000000000dffff (prio 1, i/o): alias pam-pci @pci 00000000000dc000-00000000000dffff
    ```
    ```bash
    00000000000e0000-00000000000e3fff (prio 1, i/o): alias pam-pci @pci 
    00000000000e0000-00000000000e3fff
    00000000000e4000-00000000000e7fff (prio 1, i/o): alias pam-pci @pci 00000000000e4000-00000000000e7fff
    00000000000e8000-00000000000ebfff (prio 1, i/o): alias pam-pci @pci 00000000000e8000-00000000000ebfff
     
    00000000000ec000-00000000000effff (prio 1, i/o): alias pam-pci @pci 00000000000ec000-00000000000effff
    00000000000f0000-00000000000fffff (prio 1, i/o): alias pam-pci @pci 00000000000f0000-00000000000fffff
    00000000fec00000-00000000fec00fff (prio 0, i/o): ioapic
    00000000fed00000-00000000fed003ff (prio 0, i/o): hpet
    00000000fee00000-00000000feefffff (prio 4096, i/o): apic-msi

memory-region: system
  0000000000000000-ffffffffffffffff (prio 0, i/o): system
    0000000000000000-0000000005ffffff (prio 0, ram): alias ram-below-4g @pc.ram 0000000000000000-0000000005ffffff
    0000000000000000-ffffffffffffffff (prio -1, i/o): pci
      00000000000a0000-00000000000affff (prio 2, ram): alias vga.chain4 @vga.vram 0000000000000000-000000000000ffff
      00000000000a0000-00000000000bffff (prio 1, i/o): vga-lowmem
      00000000000c0000-00000000000dffff (prio 1, rom): pc.rom
      00000000000e0000-00000000000fffff (prio 1, rom): alias isa-bios @pc.bios 00000000003e0000-00000000003fffff
      0000000080000000-0000000080ffffff (prio 1, ram): vga.vram
      0000000081010000-0000000081010fff (prio 1, i/o): vga.mmio
        0000000081010000-000000008101017f (prio 0, i/o): edid
        0000000081010400-000000008101041f (prio 0, i/o): vga ioports remapped
        0000000081010500-0000000081010515 (prio 0, i/o): bochs dispi interface
        0000000081010600-0000000081010607 (prio 0, i/o): qemu extended regs
      00000000ffc00000-00000000ffffffff (prio 0, rom): pc.bios
    ```
    ```bash
    00000000000a0000-00000000000bffff (prio 1, i/o): alias smram-region @pci 00000000000a0000-00000000000bffff
    00000000000c0000-00000000000c3fff (prio 1, ram): alias pam-rom @pc.ram 
    00000000000c0000-00000000000c3fff
    00000000000c4000-00000000000c7fff (prio 1, i/o): alias pam-pci @pci 00000000000c4000-00000000000c7fff
    00000000000c8000-00000000000cbfff (prio 1, i/o): alias pam-pci @pci 00000000000c8000-00000000000cbfff
    00000000000cc000-00000000000cffff (prio 1, i/o): alias pam-pci @pci 00000000000cc000-00000000000cffff
    00000000000d0000-00000000000d3fff (prio 1, i/o): alias pam-pci @pci 00000000000d0000-00000000000d3fff
    00000000000d4000-00000000000d7fff (prio 1, i/o): alias pam-pci @pci 00000000000d4000-00000000000d7fff
    00000000000d8000-00000000000dbfff (prio 1, i/o): alias pam-pci @pci 00000000000d8000-00000000000dbfff
    00000000000dc000-00000000000dffff (prio 1, i/o): alias pam-pci @pci 00000000000dc000-00000000000dffff
    00000000000e0000-00000000000e3fff (prio 1, i/o): alias pam-pci @pci 00000000000e0000-00000000000e3fff
    00000000000e4000-00000000000e7fff (prio 1, i/o): alias pam-pci @pci 00000000000e4000-00000000000e7fff
    00000000000e8000-00000000000ebfff (prio 1, i/o): alias pam-pci @pci 00000000000e8000-00000000000ebfff
    00000000000ec000-00000000000effff (prio 1, i/o): alias pam-pci @pci 
    00000000000ec000-00000000000effff
    00000000000f0000-00000000000fffff (prio 1, i/o): alias pam-pci @pci 00000000000f0000-00000000000fffff
    00000000fec00000-00000000fec00fff (prio 0, i/o): ioapic
    00000000fed00000-00000000fed003ff (prio 0, i/o): hpet
    00000000fee00000-00000000feefffff (prio 4096, i/o): apic-msi

memory-region: smram
    00000000000a0000-00000000000bffff (prio 0, ram): alias smram-low @pc.ram 00000000000a0000-00000000000bffff

memory-region: pc.ram
  0000000000000000-0000000005ffffff (prio 0, ram): pc.ram

memory-region: vga.vram
  0000000080000000-0000000080ffffff (prio 1, ram): vga.vram

memory-region: pc.bios
  00000000ffc00000-00000000ffffffff (prio 0, rom): pc.bios
```
```bash
memory-region: pci
  0000000000000000-ffffffffffffffff (prio -1, i/o): pci
    00000000000a0000-00000000000affff (prio 2, ram): alias vga.chain4 @vga.vram 0000000000000000-000000000000ffff
    00000000000a0000-00000000000bffff (prio 1, i/o): vga-lowmem
    00000000000c0000-00000000000dffff (prio 1, rom): pc.rom
    00000000000e0000-00000000000fffff (prio 1, rom): alias isa-bios @pc.bios 00000000003e0000-00000000003fffff
    0000000080000000-0000000080ffffff (prio 1, ram): vga.vram
    0000000081010000-0000000081010fff (prio 1, i/o): vga.mmio
      0000000081010000-000000008101017f (prio 0, i/o): edid
      0000000081010400-000000008101041f (prio 0, i/o): vga ioports remapped
      0000000081010500-0000000081010515 (prio 0, i/o): bochs dispi interface
      0000000081010600-0000000081010607 (prio 0, i/o): qemu extended regs
    00000000ffc00000-00000000ffffffff (prio 0, rom): pc.bios
```

查看内核的加载情况：
#figure(
    image("img/dbg_readelf.png",width:65%),
)
#figure(
    image("img/dbg_register.png",width:  100%),
)
=== 步骤三：回答问题
==== set_entry 函数做了什么？为什么它是 unsafe 的？
答:它的任务是“写内存”,但写的不是普通数据,而是CPU硬件识别的页表项。

其具体操作包括：

- 计算索引：根据传入的虚拟地址，提取出对应层级的索引。

- 构造数据：结合物理页框地址和位标志

- 写入内存:将构造好的64位数值写入到页表在内存中对应的偏移地址处。

将其标记为unsafe主要有以下三个原因:

- 直接操作物理内存地址：该函数需要将一个整数值（物理地址）写入到特定的内存地址，如果地址计算错误，可能会覆盖内核的代码区、堆栈，甚至修改了不该动的硬件寄存器。这种任意内存写入是极其危险的。

- 破坏内存安全语义：该函数可以通过修改页表，让两个不同的虚拟地址指向同一个物理地址或者撤销某个内存页的映射。如果该函数在程序还在引用这块内存时撤销了映射，后续的访问会导致 Page Fault 或释放后使用,这违反了Rust的安全契约。

- 触发未定义行为:修改页表直接影响CPU的硬件行为。如果该函数错误地关闭了某个页面的权限位,CPU会立即崩溃。

==== jump_to_entry 函数做了什么？要传递给内核的参数位于哪里？查询 call 指令的行为和 x86_64 架构的调用约定，借助调试器进行说明。

答：

1. jump_to_entry 到底做了什么？

从汇编层面看，它本质上是一个 绝对跳转 或 函数调用。

地址转换： 它将内核 ELF 文件的入口点地址(Entry Point)加载到寄存器中。

上下文切换： 它丢弃了 Bootloader 的栈帧,通过修改指令指针寄存器(RIP),让 CPU 开始执行内核的第一条指令。

参数准备： 在跳转之前，它必须按照 x86_64 调用约定 将预先准备好的 BootConfig 结构体指针放置在特定的寄存器中。

2. 传递给内核的参数位于哪里？
根据 System V AMD64 ABI(Linux 和大多数开源内核遵循的调用约定)，函数调用的前 6 个整数或指针参数依次通过以下寄存器传递：

RDI (第一个参数) — YatSenOS 的 BootConfig 指针通常就在这里。

RSI (第二个参数)

RDX (第三个参数)

RCX

R8

R9

因此，内核的入口函数会去 RDI 寄存器 中寻找那个指向内存映射表、帧缓冲区信息的结构体地址。

3. 深入理解 call 指令的行为

在调试器(如 GDB 或 QEMU Monitor)中观察 call 指令，你会发现它完成了两件事：

压栈 (Push RIP): 将 call 指令下一条指令的地址压入栈中（保存返回地址）。

跳转 (Jump): 将 RIP 设置为目标函数的起始地址。

注意： 在 jump_to_entry 中，有时我们会使用 jmp 而不是 call。因为内核启动后永远不会“返回”给 Bootloader,所以不需要在栈上保存返回地址。

==== entry_point! 宏做了什么？内核为什么需要使用它声明自己的入口点？

答:它是一个代码生成器(Wrapper)。当写下 entry_point!(kernel_main) 时，这个宏会在底层自动生成一段符合底层硬件和引导程序规范的样板代码。

如果不使用这个宏，直接手写一个函数，内核将面临以下三个致命问题。这也是必须使用该宏的原因：

A. 解决函数名修饰(Name Mangling)问题

问题： Rust 编译器默认会对函数名进行“混淆/修饰”(Mangling)，但链接器(Linker)和引导程序在寻找入口点时，通常只认一个固定的名字。

宏的解决方式： 宏会在生成的底层函数上自动加上 #[no_mangle] 属性，确保入口符号名在编译后原封不动，保证 Bootloader 能够精准跳转。

B. 统一调用约定(Calling Convention / ABI)

问题： Rust 语言自身的函数调用约定(Rust ABI)是不稳定的，并且与 C 语言标准不同（寄存器使用规则可能不一样）。而 Bootloader 执行跳转时（如上文提到的利用 rdi 传递参数），遵循的是标准的 System V AMD64 ABI (C ABI)。如果不用 C ABI 接收参数，内核一启动就会读错寄存器，导致崩溃。

宏的解决方式： 宏强制生成带有 extern "C" 签名的入口函数，确保内核接收参数的方式与 Bootloader 传参的方式严丝合缝。

C. 保证内存与类型安全（最核心的 Rust 特色）

问题： 在 C 语言或汇编中,入口点就是一个内存地址,你可以强行把任何东西传给它,编译器不会报错(但这会导致运行时的未定义行为)。Rust 作为一门强调安全的语言，绝不允许这种事情发生。

宏的解决方式： entry_point! 宏会在编译期进行严格的类型检查。它要求你传入的 kernel_main 函数必须满足特定的签名（通常是接收一个 &'static BootInfo 并返回 !，即永不返回）。如果你手误写成了 fn kernel_main(x: i32)，宏在编译阶段就会直接报错，将致命的内存错位问题扼杀在摇篮里。

总结

entry_point! 宏封装了所有脏活累活。它把底层那些危险的、与架构强相关的 unsafe 胶水代码隐藏了起来，让内核开发者可以安心地在一个类型安全、签名正确的纯 Rust 函数(kernel_main)中开始编写操作系统的核心逻辑。

==== 如何为内核提供直接访问物理内存的能力？你知道几种方式？代码中所采用的是哪一种？

答：

1. 内核访问物理内存的四种常见方式

A. 恒等映射 (Identity Mapping)

原理： 将物理地址 X 直接映射到虚拟地址 X。

优点： 物理地址和虚拟地址完全一样，无需计算，非常直观。

缺点： 极其浪费虚拟地址空间，且会占据低端地址（通常用于用户态程序），容易引发地址冲突。现代 OS 仅在 Bootloader 刚启动分页的那一小段时间使用。

B. 固定偏移映射 / 线性映射 (Fixed Offset Mapping / Linear Mapping)

原理： 选择一个极高的虚拟地址作为起点（即 Offset,通常远高于用户态程序的可用地址），将物理地址 X 映射到虚拟地址 X + Offset。

优点： 转换极其快速(只需加减偏移量),且能通过一个连续的虚拟地址空间直接访问所有的物理内存。64 位系统的虚拟地址空间巨大，完全放得下整个物理内存。现代 Linux 内核(通过 page_offset_base)和绝大多数 64 位教学 OS 都采用这种方法。

缺点： 需要在启动阶段就把所有物理内存映射好，占用一部分页表空间（通常使用 2MiB 或 1GiB 大页来优化）。

C. 临时映射 (Temporary Mapping)

原理： 内核保留一小块专门的虚拟地址空间。当需要访问某个物理页时，临时修改这块虚拟空间的页表项，将其指向目标物理页；访问完成后解除映射。

优点： 节省虚拟地址空间（在 32 位系统中物理内存大于虚拟地址空间时非常有用，比如早期的 Linux 高端内存 HighMem 机制）。

缺点： 极其低效。每次访问物理内存都需要修改页表并刷新 TLB。

D. 递归页表映射 (Recursive Page Table Mapping)
原理： 一种非常巧妙的技巧，将最高级页表(如 P4)的最后一个表项指向 P4 页表自身的物理地址。

适用场景： 专门用于访问和修改页表本身。

这份代码采用的是固定偏移映射：

在 map_physical_memory 函数中，遍历了从 0 到 max_addr 的所有物理帧，并为它们逐一建立映射：

```rs
// 计算出的虚拟地址 = 物理帧起始地址 + 预设的 offset
let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64() + offset));

// 将该虚拟地址页映射到该物理帧
page_table.map_to(page, frame, flags, frame_allocator)...
```

这里利用了 Size2MiB 大页，一口气把所有物理内存映射到了以 offset 为起点的高半区虚拟地址中。

在 load_segment 函数中，为了把 ELF 文件的数据拷贝到刚刚分配的物理页中，必须在开启分页的情况下往该物理页里写数据。由于直接写物理地址会崩溃，代码利用了刚才建立的偏移映射机制：

```rs
// dest_ptr 是一个可以直接解引用的虚拟地址
// 它等于：新申请的物理页地址 + 线性映射的 offset
let dest_ptr = (frame.start_address().as_u64() + physical_offset) as *mut u8;

// 然后安全地通过虚拟地址写入数据
copy_nonoverlapping(..., dest_ptr, ...);
```

==== 为什么 ELF 文件中不描述栈的相关内容？栈是如何被初始化的？它可以被任意放置吗？

答：

1. 为什么 ELF 文件中不描述栈的相关内容？

简而言之:ELF 文件是程序的“静态蓝图”，而栈是程序运行时的“动态草稿纸”。

ELF 存的是“已知”： ELF 文件中包含的代码段(.text)、数据段(.data)和只读数据段(.rodata),在编译时内容和大小就已经完全确定了。

栈处理的是“未知”： 栈是用来保存函数调用时的局部变量、返回地址和寄存器状态的。程序在运行前，根本无法预知函数会嵌套调用多少层、会产生多少局部变量。因此，把栈的数据打包进 ELF 文件既不现实，也毫无意义。

2. 栈是如何被初始化的？

在操作系统和底层开发中，栈的初始化分为两个动作：分配内存 和 拨动指针。

- 第一步：分配内存并映射

```rs
// --- 步骤 4.3: 映射内核栈 ---
elf::map_range(
    config.kernel_stack_address,
    config.kernel_stack_size,
    ...
)
```

Bootloader 通过向页表申请物理页，在虚拟内存中强行“圈”出了一块干净的空间，留作内核栈使用。

- 第二步：设置栈顶指针寄存器

内存有了,CPU 怎么知道去哪里用？在 x86_64 架构下,CPU 只认 rsp(Stack Pointer)寄存器。

```rs
let stacktop = config.kernel_stack_address + config.kernel_stack_size * 0x1000 - 8;
jump_to_entry(&bootinfo, stacktop);
```
Bootloader 计算出了这段内存的最高地址（因为 x86 架构的栈是向下生长的，从高地址向低地址蔓延），并通过汇编指令将这个 stacktop 的值塞进了 rsp 寄存器。
一旦 rsp 被赋值，栈就正式初始化完毕了！ 接下来内核执行的第一条 push 或 call 指令，就会自动把数据写进你刚刚映射的那块内存里。

3. 栈可以被任意放置吗？

理论上：可以。 只要是一块映射了物理内存且具有读写权限的虚拟地址空间，你把 rsp 指向哪里，哪里就是栈。

工程实践上：不可以随意放，必须遵守以下铁律：

- 绝不能与已有数据重叠： 如果你把栈放在了代码段或者数据段附近，随着函数的深入调用，栈向下生长，就会悄无声息地覆盖掉你的内核代码或全局变量，导致系统极其诡异地崩溃。

- 必须满足对齐要求： 根据 System V AMD64 ABI,在执行 call 指令进入普通 C/Rust 函数之前，栈指针 rsp 必须是 16 字节对齐的。不对齐会导致使用 SSE/AVX 向量指令（如浮点运算）时直接触发硬件异常。

- 留出安全距离(Guard Page): 为了防止“栈溢出(Stack Overflow)”破坏其他内存，成熟的 OS 通常会在栈的最低地址处放置一个不可读写的空页(Guard Page)。这样一旦栈生长过头碰到了这个页,CPU 会立刻触发 Page Fault 被内核捕获，而不是任由其破坏周围的数据。

==== 请解释指令 layout asm 的功能。倘若想找到当前运行内核所对应的 Rust 源码，应该使用什么 GDB 指令？

layout asm 是 GDB TUI 模式下的一个命令，其全称是 Layout Assembly(汇编视图)。

核心功能： 当你在 GDB 中输入这个指令后，终端终端的上方会分出一个独立的窗口，实时显示当前正在执行的汇编代码。

动态追踪： 窗口中会高亮显示程序计数器(rip 寄存器)当前指向的那条汇编指令。当你使用 si(单步执行指令)或 ni 往下走时，高亮条会跟着实时移动。

适用场景： 在 OS 开发中，当你刚跳入内核入口点,或者在调试上下文切换、中断处理等纯汇编逻辑时,layout asm 能让你像拥有“X 光眼”一样，清晰地看到 CPU 真正在执行什么机器指令。

为了找到当前运行内核所对应的 Rust 源码，我使用了```rs layout split ```指令,这时GDB会将屏幕分为三个区域:上方显示Rust源码,中间显示对应的汇编代码,下方是命令行输入区。

#figure(
    image("img/dg.png",width :54%),
)

==== 假如在编译时没有启用 DBG_INFO=true,调试过程会有什么不同?

答：

编译过程的本质，是将具备高度抽象语义（如变量类型、作用域、函数边界）的源代码，降维映射为抹除了所有结构化特征的线性机器指令与绝对内存地址。在此过程中启用 DBG_INFO=true(即向生成的 ELF 文件中注入 DWARF 等格式的调试符号),其核心技术意义在于构建并保留了一个元数据映射层(Metadata Mapping Layer)。

当调试器接管包含调试符号的程序时，其执行的是基于语义的逆向解析：
调试器通过解析 ELF 文件中的 .debug_info、.debug_line 等特殊节区，将处理器当前指令指针所指向的十六进制物理/虚拟地址,精准映射回源文件中的对应行号。同时,利用符号表中静态保留的类型签名(Type Signature),调试器能够自动计算内存偏移量并执行数据反序列化,使开发者得以直接观测结构化的高级语言抽象数据模型。此外,依靠调用帧信息(CFI, Call Frame Information),调试器能够可靠地进行栈回溯(Stack Unwinding),还原精确的函数调用关系链。

反之,若在编译时剥离调试信息,调试过程将发生严重的语义降级(Semantic Degradation):
由于元数据映射层完全丢失，调试器被迫退化为纯硬件视角的指令集观测工具。原本具名的函数入口退化为匿名的跳转地址，类型安全的数据结构退化为无语义标识的连续字节流。在此模式下，调试工具丧失了语义重构的自动化支持能力，迫使开发者必须手动介入，严格依据底层架构规范与系统级应用二进制接口(如 System V AMD64 ABI),通过人脑推演寄存器寻址规律以及栈基址指针与栈顶指针的相对偏移，以此在机器码的荒原中人工逆向复原程序的控制流与数据流。

==== 你如何选择了你的调试环境?截图说明你在调试界面(TUI 或 GUI)上可以获取到哪些信息?

我构建的调试环境采用了系统级开发中最经典的 “双机调试”(Remote Debugging)架构：

执行端(QEMU 硬件模拟器): 作为被调试的“靶机”。通过传入 -s -S 参数,QEMU 在启动时不仅模拟了完整的 x86_64 硬件环境，还在底层开启了一个 GDB Server(监听 1234 端口)，并在 CPU 执行第一条指令前将其冻结。

控制端(GDB 调试器): 作为调试的“宿主机”。它加载了带有完整调试符号表(Debug Info)的 ELF 文件，并通过 TCP 协议连接到 QEMU。此时,GDB 拥有了对虚拟 CPU 的绝对控制权。

在这个调试界面上，可以看到以下五个维度的核心信息：

- 控制流与代码映射层：Rust 源码视图、汇编指令和调用栈回溯。

- 寄存器状态

- 内存观测：栈帧数据、堆与全局变量、直接内存转储

- 硬件寻址与页表结构

- 调试控制状态

== UART 与日志输出

=== 步骤一：知识学习

- 串口：一种常见的计算机接口，用于在计算机和外部设备之间进行串行数据传输。串口是一种通用的调试接口，几乎所有的计算机和嵌入式设备都提供了串口接口。它提供了低级别的硬件访问能力，可以直接与设备进行通信。因此，串口通常用于低级别的系统调试和硬件调试，例如在操作系统启动之前或操作系统不可用的情况下进行调试。

总而言之，串口本质上是一个最原始的数据通道，对于正在开发的操作系统，它之所以如此重要，主要有以下三大原因：

1. 不需要复杂的驱动

如果要往屏幕打印一个字符：需要初始化显卡、配置显存帧缓冲、加载字体库、计算像素位置，甚至还要写一个完整的驱动程序。这在内存刚启动、连内存分配器都还没写好的早期阶段，是不可能的。

如果要往串口打印一个字符：在X86架构下，只需要一条极其简单的汇编指令。标准的COM1串口被硬编码在CPU的I/O端口0x3F8上。只需要把字符塞进这个端口，硬件芯片(UART)就会自动帮你把它发送出去。

2. 内核的调试窗口

因为向串口发送数据极度依赖底层硬件，且完全不依赖操作系统的其他高级子系统，它成为了系统崩溃时的最佳汇报工具。当内核发生严重的 Page Fault 或 Panic ，导致整个系统锁死、屏幕直接卡或黑屏时，串口通常依然能工作。内核开发者会在崩溃前一瞬间，把寄存器状态和错误日志往串口里塞。这些信息就是排查机器死机的唯一线索。

3. QEMU中的“幽灵串口”

在QEMU虚拟机里，一切硬件都是用软件模拟出来的。

- UART：在串行交互界面上负责对数据完成编解码的硬件芯片。它采用异步通信和全双工通信。芯片的核心工作是做“并转串”和“串转并”的翻译官。

- COM：主板上通常会预留几个UART硬件接口。为了方便软件去找到它们，系统给它们起了固定名字：COM1、CON2等。更重要的是，在x86架构下，COM1对应着一个固定的 I/O 端口基地址：0x3F8.

- x86 I/O 端口

CPU 与计算机外部I/O设备的常见交互模式分为 内存映射I/O(MMIO) 和端口映射I/O(PIO) 两种。

Memory-mapped I/O (MMIO) 即通过将需要进行交互的 I/O 设备的相关寄存器映射到某一段内存地址空间，从而实现对 I/O 设备的访问。在启用虚拟内存机制的系统中，这些内存空间同样需要通过虚拟地址进行访问。

Port-mapped I/O 即将 I/O 设备的相关寄存器编址在相对与内存地址独立的地址空间，并使用专门的指令与 I/O 设备进行交互。在 x86 系统中，I/O 端口的地址空间为 0x0000 - 0xFFFF，可以通过 in 和 out 指令进行访问。

=== 步骤二：串口驱动

==== 在考虑 IO 设备驱动（SerialPort）的设计时，需要考虑如下问题：

===== 为了描述驱动的状态，需要存储哪些数据？

答：轮询模式下只需要存储一个基址端口号(如0x3F8)；中断与缓冲模式下还需要增加软件层面的状态数据，例如发送/接收环形缓冲区、初始化标志位。

===== 需要如何与硬件进行交互？

答：操作系统必须跨越软件的边界吗，直接对硬件发出电信号。在x86架构中，主要通过端口映射 I/O 来完成：

- 配置与控制：通过向特定偏移的端口(如基址+1、基址+3)写入控制字，来指挥硬件的行为。比如开启 DLAB 标志位、设置波特率除数、配置 8N1 数据格式。

- 状态探测：硬件处理速度远比 CPU 慢。在发送数据前，必须读取线路状态寄存器，检查发送缓冲区是否为空；在接受数据前，检查是否有数据就绪。

- 数据读写： 使用底层的 inb 和 outb 汇编指令。在Rust中，这通常被安全地封装成了Port::read() 和 Port::write() 方法。

===== 与硬件交互的过程中，需要考虑哪些并发问题？

答：

- 多核/多线程竞争：必须使用互斥锁将整个驱动对象包裹起来。获取锁才能发送，发完才能释放。

- 中断引发的死锁：在获取串口锁之前，必须暂时关闭当前 CPU 的本地中断。等打印完毕释放锁后，再恢复中断状态。

===== 驱动需要向内核提供哪些接口？

答：优秀的驱动程序应该把端口计算和硬件轮询藏在里面，向内核提供干净、高级的抽象接口：

- 生命周期接口

init()：供内核在极早期的启动阶段调用，唤醒并配置硬件。

- 底层原语接口

send_byte(u8) / receive_byte() -> Option<u8>：提供最基础的单字节吞吐能力。

- 高级格式化接口

实现 core::fmt::Write trait： 这是 Rust 内核生态的点睛之笔。只要实现了 write_str 方法，驱动就能与 Rust 强大的格式化引擎无缝对接。内核的其他模块只需要愉快地使用 println! 或 info!("value: {}", x)，底层的转换、缓冲、发往端口的操作全部由驱动自动代劳。

==== 完成 uart16550 驱动：

```rs
pub fn init(&self) {
        let mut line_control_port = Port::new(self.port + 3);
        let mut fifo_control_port = Port::new(self.port + 2);
        let mut interrupt_enable_port = Port::new(self.port + 1);
        let mut modem_control_port = Port::new(self.port + 4);

        unsafe {
            // 1. 禁用所有中断
            interrupt_enable_port.write(0x00u8);

            // 2. 设置波特率 (115200)
            // 开启 DLAB (Divisor Latch Access Bit)
            line_control_port.write(0x80u8);
            // 设置分频器 (115200 波特率的分频值为 1)
            Port::<u8>::new(self.port).write(0x01u8);      // 低 8 位
            Port::<u8>::new(self.port + 1).write(0x00u8);  // 高 8 位

            // 3. 设置数据格式: 8 数据位, 无校验, 1 停止位
            line_control_port.write(0x03u8);

            // 4. 启用并重置 FIFO 缓冲区
            fifo_control_port.write(0xC7u8);

            // 5. 设置调制解调器控制位 (DTR, RTS, Out2)
            modem_control_port.write(0x0Bu8);
        }
    }

```

```rs
/// Sends a byte on the serial port.
    pub fn send(&mut self, data: u8) {
        let mut status_port = Port::<u8>::new(self.port + 5);
        let mut data_port = Port::<u8>::new(self.port);

        unsafe {
            // 关键：轮询检查 Line Status Register (LSR)
            // 检查第 5 位 (0x20) 是否为 1 (Transmitter Holding Register Empty)
            // 只有当缓冲区为空时，才能安全地写入下一个字节
            while (status_port.read() & 0x20) == 0 {
                core::hint::spin_loop();
            }
            // 向数据寄存器写入字节
            data_port.write(data);
        }
    }
```

```rs
/// Receives a byte on the serial port no wait.
    pub fn receive(&mut self) -> Option<u8> {
        let mut status_port = Port::<u8>::new(self.port + 5);
        let mut data_port = Port::<u8>::new(self.port);

        unsafe {
            // 检查第 0 位是否为 1 (Data Ready)
            if (status_port.read() & 1) == 0 {
                None
            } else {
                Some(data_port.read())
            }
        }
    }
```
在调用 drivers::serial::init() 后，能正常看到```bash [+] Serial Initialized. ```的输出，说明串口驱动已经成功初始化。

#figure(
    image("img/serial_init.png",width:100%)
)

=== 步骤三：日志输出
补充下述代码：

```rs
use log::{Level, Metadata, Record};

pub fn init() {
    static LOGGER: Logger = Logger;
    log::set_logger(&LOGGER).unwrap();

    // 配置日志框架的最大输出级别：
    // 在内核开发的早期阶段，设置为 Trace，这样就能看到包括底层的每一步调试信息。
    log::set_max_level(log::LevelFilter::Trace);

    info!("Logger Initialized.");
}
```
```rs
struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // 判断当前拦截到的日志级别，是否低于或等于我们全局设定的最大级别
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {   
            // 为不同级别分配终端控制台颜色
            let color_code = match record.level() {
                Level::Error => 31,
                Level::Warn => 33,
                Level::Info => 32,
                Level::Debug => 36,
                Level::Trace => 90,
            };
        
            // 提取产生该日志的代码文件路径和行号
            // 使用 unwrap_or 提供默认值，防止偶尔获取不到信息导致内核 Panic
            let file = record.file_static().unwrap_or("unknown");
            let line = record.line().unwrap_or(0);

            // 组合并输出日志
            // \x1b[{}m  : 告诉终端接下来的字用什么颜色显示
            // \x1b[0m   : 打印完毕后，把终端颜色重置回默认状态，防止后续全变色
            // {:>5}     : 让日志级别（如 INFO, WARN）右对齐，占5个字符宽度，显得整齐美观
            println!(
                "\x1b[{}m[{:>5}] [{}:{}] {}\x1b[0m",
                color_code,
                record.level(),
                file,
                line,
                record.args()
            );
        }
    }

    fn flush(&self) {}
}
```
#figure(
    image("img/color.png",width: 100%)
)

如图所示，内核的日志实现了彩色输出。

=== 步骤四：Panic处理

在 kernel/src/main.rs任意地方加上一段代码出现panic:

```rs
    panic!("Don't panic! This is just a test for YatSenOS.");
```
#figure(
    image("img/panic.png",width:61%),
)

== 思考题

=== 在根目录的 Cargo.toml 中，指定了依赖中 boot 包为 default-features = false，而它会被内核引用，禁用默认 feature 是为了避免什么问题？请结合 crates/boot 的 Cargo.toml 谈谈你的理解。

1. boot 包的“双重身份”

身份 A：作为独立的引导程序（Executable）
当你运行编译命令构建 Bootloader 时，boot 被编译为一个独立的 .efi 可执行文件。此时，它需要与主板的 UEFI 固件深度交互，调用 UEFI 提供的屏幕输出、磁盘读取、文件系统（如你的 fs.rs）和内存分配等服务。

身份 B：作为内核的数据结构接口库（Library）

当内核启动后，它需要知道 Bootloader 走之前给它留了什么“遗产”（比如内存映射表的物理地址、屏幕帧缓冲区的地址、内核命令行参数等）。这些数据的结构体定义（例如你代码里用到的 BootInfo 和 config）就写在 boot 包里。因此，内核（kernel）必须把 boot 作为一个普通库引入，以便正确解析这些数据结构。

2. 结合 boot 的 Cargo.toml 来看：默认 Feature 里藏了什么？

UEFI 相关的底层 Crate： 用于在引导阶段调用固件接口（比如 uefi 库）。

专门的图形/日志支持： 用于在 UEFI 阶段打印白色的提示信息。

特定的全局分配器和 Panic 处理器： 依赖于 UEFI 运行时的机制。

3. 如果内核不禁用默认 Feature，会引发什么灾难？

当 kernel 在其 Cargo.toml 中引入 boot 时，如果不加上 default-features = false，Cargo 强大的特性合并（Feature Unification）机制就会把 boot 的 UEFI 依赖和执行逻辑全部打包进内核中。这将引发三大致命冲突：

💥 灾难一：编译目标（Target）不兼容

 Bootloader 是编译给 x86_64-unknown-uefi 这个目标的，它依赖 UEFI 固件。而内核是编译给纯裸机目标 x86_64-unknown-none 的。

如果内核拉取了 boot 的默认功能，也就拉取了 UEFI 相关的 crate。而这些 crate 根本无法在 none（无操作系统环境）的目标下编译，直接导致内核在编译阶段报出一堆平台不兼容的错误。

💥 灾难二：“天无二日”的语义项冲突 (Lang Items Clash)


boot 包（默认开启时）为了自己能独立运行，内部可能定义了一套自己的 #[panic_handler] 或引入了全局的内存机制。

kernel 包也定义了自己的 #[panic_handler]（就是你刚改过的彩色报错）和 #[global_allocator]（之前的代码里写的 DummyAllocator）。

如果在内核里引入了 boot 的默认特征，编译器会立刻崩溃，提示发现重复的 Panic Handler 或 Allocator。

💥 灾难三：运行时的“逻辑毒药”

正如之前在截图里看到的 Exiting boot services...，一旦 Bootloader 跳转到内核，UEFI 固件的服务就已经被彻底关闭并销毁了。如果内核里包含了 boot 中调用 UEFI 服务的代码，哪怕它只是个没被执行的死代码，一旦意外触发或链接器处理不当，CPU 就会试图去调用一个已经不存在的固件地址，导致瞬间且毫无征兆的系统死机。

=== 在 crates/boot/src/main.rs 中参考相关代码，聊聊 max_phys_addr 是如何计算的，为什么要这么做？

==== 什么是max_phys_addr?

max_phys_addr 是处理器物理地址空间中，需要被直接映射覆盖的最高物理地址边界。计算max_phys_addr的根本目的，是为了在操作系统启动早期安全、高效地构建“物理内存的线性映射”页表体系。

计算它的必要性体现在以下三个方面：

- 确定线性映射的页表构建边界：在现代 x86_64 操作系统架构（如 Linux 以及你正在开发的 YatSenOS）中，内核通常会将整个可用的物理地址空间统一映射到一个固定的高位虚拟地址偏移处（即 Virtual Address = Physical Address + OFFSET）。

Bootloader 负责在跳转到内核主函数（kernel_main）之前，建立好这套基础的四级页表（PML4）。为了建立这套映射，代码需要执行内存分配并填写页表项（PTE）。max_phys_addr 充当了建表逻辑的循环上限边界。它指示了 Bootloader 必须为 [0, max_phys_addr) 这个闭环范围内的所有物理地址分配虚拟地址映射。

- 防止内存资源的过度浪费：既然需要建立线性映射，为什么不直接把架构支持的理论最大值全部映射了？

在 x86_64 架构下，物理地址上限可以达到 48 位（256TB）甚至 52 位（4PB）。即使采用 2MB 或 1GB 的大页（Huge Pages）进行映射，盲目映射整个理论空间依然会消耗惊人数量的连续物理内存，仅仅用于存放页表本身。

通过动态解析 UEFI Memory Map 并计算出真实的 max_phys_addr，Bootloader 能够做到按需建表。如果当前硬件环境的物理边界在 8GB，页表构建逻辑在映射完这 8GB 后就会主动停止。这避免了页表过度膨胀，将宝贵的物理内存保留给内核态的堆分配器（Heap Allocator）以及未来的用户态进程使用。

- 规避内核态缺页异常：如果 Bootloader 偷懒，不计算精确的最大值，而是随便估算一个较小的值（例如仅映射前 1GB），会导致灾难性后果。

内核在初始化各种底层外设驱动时，必须通过 OFFSET + 物理基址 形成的虚拟地址来操作设备寄存器（如 PCIe 设备、显卡、APIC 等高位 MMIO 区域）。如果线性映射因为没有精确计算 max_phys_addr 而未能覆盖这些高位物理地址，当 CPU 的指令指针执行到读写设备寄存器的代码时，MMU（内存管理单元）将在页表树中找不到对应的有效映射（Present Bit 为 0）。

这会立刻触发缺页异常（Page Fault）。在内核启动初期，如果触发由于映射不全导致的缺页异常，通常属于致命错误，会直接导致内核崩溃（Panic）。精确计算 max_phys_addr 是确保内核对所有硬件拥有合法访问权的前提。

==== max_phys_addr 如何计算？

```rs
let mmap = uefi::boot::memory_map(...); // 从主板 UEFI 固件获取物理内存分布表

let max_phys_addr = mmap
    .entries() // 1. 遍历内存映射表中的每一个条目 (Memory Descriptor)
    .map(|m| m.phys_start + m.page_count * 0x1000) // 2. 计算每块内存的结束地址
    .max() // 3. 找出所有块中，结束地址最大的那一个
    .unwrap_or(0x1_0000_0000) // 4. 防御性编程：如果没拿到数据，默认给 4GB
    .max(0x1_0000_0000); // 5. 强制托底：就算最大物理地址不足 4GB，也强行拔高到 4GB
```
==== 为什么要这么做？

- 为了建立完整的“线性物理映射”：内核开启分页后，为了能任意分配和读写物理内存，Bootloader 会利用一个极高虚拟地址作为偏移量（physical_memory_offset），将整块物理内存一口气映射进虚拟地址空间。

但是，如果不遍历 UEFI 内存表，Bootloader 怎么知道物理内存到底有多大呢？

如果映射少了，内核以后分配到高地址的物理页帧时，就会因为没有映射而触发 Page Fault 崩溃。因此，必须找出真正的最高物理地址，一次性映射到底。

- 为什么要强制拉高到至少4GB？

这是 x86 架构极其特殊的一点：物理内存地址空间 不等于 真实的内存条（RAM）空间。

在 4GB 以下的物理地址空间里，主板偷偷藏了很多硬件设备的寄存器。这种技术叫做 MMIO (Memory-Mapped I/O, 内存映射 I/O)。

例如，高级可编程中断控制器 (APIC) 的默认物理地址在 0xFEE00000。

PCIe 设备的配置空间、显卡的显存（Framebuffer）等，通常都被插在 3GB ~ 4GB 之间的这段“空洞”（PCI Hole）里。

如果你在 QEMU 里只给虚拟机分配了 128MB 的内存，那么 mmap 算出来的实际最大 RAM 地址也就是一百多兆。如果不强制拉高到 4GB，Bootloader 就只会映射这 128MB 的范围。结果就是：当你的内核启动后，想要去读写 APIC 中断控制器或者向显存里画图时，它去访问 3GB 多位置的物理地址，直接就 Page Fault 炸了。

=== 串口驱动是在进入内核后启用的，那么在进入内核之前，显示的内容是如何输出的？

在进入内核之前（也就是 Bootloader 运行阶段），你能在终端看到的那些白色的 [ INFO] 日志，完全是依赖于 UEFI 固件（Firmware）提供的标准服务。

在计算机上电后、你的内核正式接管硬件之前，主板上的 UEFI 固件其实已经为你搭建好了一个临时的、功能相对完备的“微型运行环境”。在这个阶段，日志输出的技术实现路径与内核态截然不同：

1. UEFI System Table (系统表) 与 ConOut
当你的 Bootloader 被 UEFI 固件加载并启动时（即进入 efi_main 函数时），固件会向 Bootloader 传递一个极其核心的数据结构：UEFI System Table。

这个系统表里封装了大量固件级别的服务指针，其中就包含了一个名为 ConOut（Console Output）的指针。该指针指向了 UEFI 规范中定义的 Simple Text Output Protocol (简单文本输出协议)。

2. OutputString 函数调用
在这个协议中，UEFI 提供了一个名为 OutputString 的标准函数。
在你的 crates/boot/src/main.rs 中，当你调用 info!("Running UEFI bootloader..."); 时，Rust 的 uefi crate 在底层做了这样一件事：
它将 Rust 的字符串转换成 UEFI 规范要求的 UTF-16 字符数组，然后调用 UEFI 固件提供的 ConOut->OutputString 接口。

此时，真正负责和硬件打交道（比如把字符渲染到屏幕像素上，或者将数据转发给 QEMU 终端）的，是主板固件本身（在你的实验环境中，就是 QEMU 加载的那个 OVMF.fd 虚拟固件文件）。Bootloader 只是一个“发号施令者”，并不直接操作底层硬件。

3. “过河拆桥”：Exit Boot Services
这种“借用固件功能”的便利是临时且脆弱的。

在 Bootloader 的最后，你会看到这样一行极其关键的代码：

```rs
uefi::boot::exit_boot_services(...)
```

这行代码的系统级语义是：“Bootloader 的引导任务已完成，操作系统内核即将全面接管物理内存和硬件权限。”

一旦调用这个函数，UEFI 固件就会执行自我销毁与内存回收。包括 ConOut 在内的绝大多数 UEFI 服务及其占用的内存，都会被瞬间剥离。从这一毫秒开始，OutputString 函数彻底不复存在。

=== 在 QEMU 中，我们通过指定 -nographic 参数来禁用图形界面，这样 QEMU 会默认将串口输出重定向到主机的标准输出。

==== 假如我们将 Makefile 中取消该选项，QEMU 的输出窗口会发生什么变化？请观察指令 make run QEMU_OUTPUT= 的输出，结合截图分析对应现象。

当你在 Makefile 中取消该选项（相当于剥离了 QEMU 的无头模式）后，整个系统的输出流向会发生根本性的重定向。你会观察到以下两个极其显著的现象：

弹出一个独立的图形化窗口（VGA 显示器）： QEMU 会启动其默认的图形前端（如 GTK、SDL 或 Cocoa）。在这个弹出的窗口中，你会看到 UEFI 固件（TianoCore/OVMF）的启动 Logo，随后可能会闪过 Bootloader 阶段调用 UEFI 接口打印的白色文本。然而，当屏幕上打印出 Exiting boot services... 并正式跃迁入内核后，这个图形界面的画面将永远定格，或者直接黑屏。

宿主机终端陷入死寂： 你敲下启动指令的那个原始终端里，将再也看不到之前那绚丽的带有 ANSI 颜色的内核日志。除了 QEMU 启动时的一些基础提示外，终端没有任何后续输出。

结合内核原理的深度分析：
产生这种割裂现象的核心原因在于输出通道的不匹配。

图形窗口为什么卡死？ 当系统退出 UEFI 服务后，Bootloader 阶段借用的屏幕绘制接口被彻底销毁。由于你的 YatSenOS 目前只写了串口驱动，还没有编写显卡驱动（Framebuffer / VGA），内核根本不知道如何往屏幕的像素阵列里写字。因此，QEMU 模拟的显示器永远停留在 UEFI 移交控制权前的那一帧画面。

终端为什么没字了？ 内核的 println! 是硬编码写入物理端口 0x3F8（串口 COM1）的。在带有 -nographic 参数时，QEMU 会贴心地把 COM1 的数据线自动接到你启动它的物理终端上。但一旦移除了该参数，QEMU 默认会把虚拟机的串口映射到一个虚拟控制台（Virtual Console，通常可以在图形窗口按 Ctrl+Alt+2 或 Ctrl+Alt+3 切换查看），切断了与宿主机终端的标准输出（stdout）的联系。你的内核依然在拼命往串口发数据，只是你盯着的物理终端没有插上接收的“数据线”。

==== 在移除 -nographic 的情况下，如何依然将串口重定向到主机的标准输入输出？请尝试自行构造命令行参数，并查阅 QEMU 的文档，进行实验。

1. 把 Makefile 文件的```rs QEMU_OUTPUT := -nographic ```改成```rs QEMU_OUTPUT := -serial stdio ```

2.执行QEMU启动指令:

```bash qemu-system-x86_64 -bios OVMF.fd -drive format=raw,file=fat:rw:esp -serial stdio ```

3.可以观察到两种并发的系统状态：宿主机桌面环境和宿主机终端都会显示调试界面。

#figure(
    image("img/dbg_both.png",width:110%),
)

==== 如果你使用 ysos.py 来启动 qemu，可以尝试修改 -o 选项来实现上述功能.

1. 用```bash python ysos.py launch --help ```来查看```bash -o ```参数，结果显示如下：

```bash
(base) je-suis-un-chat@LAPTOP-MAGCR3QA:~/YatSenOS/lab1$ python  ysos.py launch --help
usage: ysos.py [-h] [-d] [-i] [-m MEMORY] [-o OUTPUT] [-p {release,debug}] [-v] [--dry-run] [--bios BIOS] [--boot BOOT]
               [--debug-listen DEBUG_LISTEN] [--vvfat_disabled]
               {build,clean,launch,run,clippy}

Build script for YSOS

positional arguments:
  {build,clean,launch,run,clippy}
                        Task to execute

options:
  -h, --help            show this help message and exit
  -d, --debug           Enable debug for qemu
  -i, --intdbg          Enable interrupt output for qemu
  -m, --memory MEMORY   Set memory size for qemu, default is 96M
  -o, --output OUTPUT   Set output for qemu, default is -nographic
  -p, --profile {release,debug}
                        Set build profile for kernel
  -v, --verbose         Enable verbose output
  --dry-run             Enable dry run
  --bios BIOS           Set BIOS path
  --boot BOOT           Set boot path
  --debug-listen DEBUG_LISTEN
                        Set listen address for gdbserver
  --vvfat_disabled      QEMU doesn't support vvfat
(base) je-suis-un-chat@LAPTOP-MAGCR3QA:~/YatSenOS/lab1$ 
```

2.在终端中执行：

```bash 
python ysos.py launch -o "-serial stdio" --vvfat_disabled
```

即可实现将串口重定向到主机的标准输入输出。

