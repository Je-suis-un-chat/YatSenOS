// 关键点：这里的 "../" 表示“退回上一级目录”，
// 也就是从 lab0 退回到 YatSenOS，然后再进入 base 文件夹寻找模板
#import "/base/report.typ": *

#show: report.with(
  title: "操作系统实验报告",
  subtitle: "实验零：环境搭建与实验准备",
  name: "郭盈盈",
  stdid: "24312063",
  classid: "吴岸聪老师班",
  major: "保密管理",
  school: "计算机学院",
  time: "2025 学年第二学期",
  banner: "./images/sysu.png"
)




= 实验目的
1. Rust 学习和巩固，了解标准库提供的基本数据结构和功能。
2. QEMU 与 Rust 环境搭建，尝试使用 QEMU 启动 UEFI Shell。
3. 了解 x86 汇编、计算机的启动过程，UEFI 的启动过程，实现 UEFI 下的 Hello, world!。

= 实验内容
1. 配置实验环境
2. 尝试使用 Rust 进行编程：五个基础任务
3. 运行 UEFI Shell：初始化仓库，使用 QEMU 启动 UEFI Shell
4. YSOS 启动：配置 Rust ToolChain，运行第一个 UEFI 程序


= 实验过程
== 配置实验环境
=== 安装项目开发环境
1.安装WSL2
```bash
 wsl --install -d Ubentu
```
2.使用系统包管理器安装依赖
#figure(
  image("screenshop1.png", width: 70%),
  caption: [验证相关软件包的版本]
)

3.安装 Rust 开发环境与工具链

1.安装 Rustup

```bash curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh```

2.安装 Rust Toolchain

```bash rustup toolchain install stable-x86_64-unknown-uefi```

3.安装 Rust 组件

```bash rustup component add rust-src --toolchain stable-x86_64-unknown-uefi```

```bash rustup component add llvm-tools-preview --toolchain stable-x86_64-unknown-uefi```

4.检查编译器版本

```bash rustc --version```

```bash cargo --version```  

#figure(
  image("screenshop2.png", width: 70%),
)