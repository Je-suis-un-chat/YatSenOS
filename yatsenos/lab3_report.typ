#import "../template/report.typ": *
#show raw.where(block: true): set block(breakable: true)


#show: report.with(
  title: "操作系统实验报告",
  subtitle: "实验三：内核线程与缺页异常",
  name: "郭盈盈",
  stdid: "24312063",
  classid: "吴岸聪老师班",
  major: "保密管理",
  school: "计算机学院",
  time: "2025 学年第二学期",
  banner: "./images/sysu.png"
)
= 实验目的

1. 了解进程与线程的概念、相关结构和实现。

2. 实现内核线程的创建、调度、切换。（栈分配、上下文切换）

3. 了解缺页异常的处理过程，实现进程的栈增长。

= 实验内容

== 进程模型设计
=== 进程控制块

= 实验过程 
