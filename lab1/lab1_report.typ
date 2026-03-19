#import "../base/report.typ": *


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