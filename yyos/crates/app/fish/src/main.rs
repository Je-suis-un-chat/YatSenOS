#![no_std]
#![no_main]

extern crate lib;

use lib::*;

const ROUNDS: usize = 20;

static SEM_GT: Semaphore = Semaphore::new(0x3000);
static SEM_LT: Semaphore = Semaphore::new(0x3001);
static SEM_UNDER: Semaphore = Semaphore::new(0x3002);
static SEM_DONE: Semaphore = Semaphore::new(0x3003);
static PRINT_LOCK: Semaphore = Semaphore::new(0x3004);

fn main() -> isize {
    assert!(SEM_GT.init(0));
    assert!(SEM_LT.init(0));
    assert!(SEM_UNDER.init(0));
    assert!(SEM_DONE.init(0));
    assert!(PRINT_LOCK.init(1));

    let gt_pid = sys_fork();
    if gt_pid == 0 {
        gt_worker();
        sys_exit(0);
    }

    let lt_pid = sys_fork();
    if lt_pid == 0 {
        lt_worker();
        sys_exit(0);
    }

    let under_pid = sys_fork();
    if under_pid == 0 {
        under_worker();
        sys_exit(0);
    }

    for round in 0..ROUNDS {
        if round % 2 == 0 {
            print_group_lt_gt_lt();
        } else {
            print_group_gt_lt_gt();
        }

        delay((round % 5) + 1);
    }

    sys_wait_pid(gt_pid);
    sys_wait_pid(lt_pid);
    sys_wait_pid(under_pid);

    println!();

    assert!(SEM_GT.remove());
    assert!(SEM_LT.remove());
    assert!(SEM_UNDER.remove());
    assert!(SEM_DONE.remove());
    assert!(PRINT_LOCK.remove());

    0
}

fn gt_worker() {
    for _ in 0..gt_count() {
        SEM_GT.wait();
        put_char(">");
    }
}

fn lt_worker() {
    for _ in 0..lt_count() {
        SEM_LT.wait();
        put_char("<");
    }
}

fn under_worker() {
    for _ in 0..ROUNDS {
        SEM_UNDER.wait();
        put_char("_");
    }
}

fn put_char(s: &str) {
    PRINT_LOCK.wait();
    print!("{}", s);
    PRINT_LOCK.signal();

    SEM_DONE.signal();
}

fn print_group_lt_gt_lt() {
    print_one(&SEM_LT);
    print_one(&SEM_GT);
    print_one(&SEM_LT);
    print_one(&SEM_UNDER);
}

fn print_group_gt_lt_gt() {
    print_one(&SEM_GT);
    print_one(&SEM_LT);
    print_one(&SEM_GT);
    print_one(&SEM_UNDER);
}

fn print_one(sem: &Semaphore) {
    sem.signal();
    SEM_DONE.wait();
}

const fn gt_count() -> usize {
    let lt_gt_lt_groups = (ROUNDS + 1) / 2;
    let gt_lt_gt_groups = ROUNDS / 2;

    lt_gt_lt_groups + gt_lt_gt_groups * 2
}

const fn lt_count() -> usize {
    let lt_gt_lt_groups = (ROUNDS + 1) / 2;
    let gt_lt_gt_groups = ROUNDS / 2;

    lt_gt_lt_groups * 2 + gt_lt_gt_groups
}

#[inline(never)]
fn delay(times: usize) {
    for _ in 0..times {
        for _ in 0..0x1000 {
            core::hint::spin_loop();
        }
    }
}

entry!(main);
