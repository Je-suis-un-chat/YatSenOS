#![no_std]
#![no_main]

use lib::*;


extern crate lib;

const THREAD_COUNT: usize = 8;
static mut COUNTER: isize = 0;

static SPIN_LOCK: SpinLock = SpinLock::new();
static SEMAPHORE: Semaphore = Semaphore::new(0x1234);

fn run_workers(worker: fn()){
    let mut pids = [0u16; THREAD_COUNT];

    for i in 0..THREAD_COUNT{
        let pid = sys_fork();

        if pid == 0{
            worker();
            sys_exit(0);
        }
        pids[i] = pid;
    }

    for pid in pids{
        sys_wait_pid(pid);
    }
}
fn do_counter_inc() {
    for _ in 0..100 {
        // FIXME: protect the critical section
        inc_counter();
    }
}

/// Increment the counter
///
/// this function simulate a critical section by delay
/// DO NOT MODIFY THIS FUNCTION
fn inc_counter() {
    unsafe {
        delay();
        let mut val = COUNTER;
        delay();
        val += 1;
        delay();
        COUNTER = val;
    }
}

fn do_counter_inc_spin(){
    for _ in 0..100{
        SPIN_LOCK.acquire();
        inc_counter();
        SPIN_LOCK.release();
    }
}

fn do_counter_inc_semaphore()
{
    for _ in 0..100{
        SEMAPHORE.wait();
        inc_counter();
        SEMAPHORE.signal();
    }
}

#[inline(never)]
#[unsafe(no_mangle)]
fn delay() {
    for _ in 0..0x100 {
        core::hint::spin_loop();
    }
}

fn test_spin() {
    unsafe {
        COUNTER = 0;
    }

    run_workers(do_counter_inc_spin);

    println!("SpinLock counter: {}", unsafe { COUNTER });
    assert_eq!(unsafe { COUNTER }, 800);
}

fn test_semaphore() {
    unsafe {
        COUNTER = 0;
    }

    assert!(SEMAPHORE.init(1));

    run_workers(do_counter_inc_semaphore);

    println!("Semaphore counter: {}", unsafe { COUNTER });
    assert_eq!(unsafe { COUNTER }, 800);

    assert!(SEMAPHORE.remove());
}

fn main() -> isize {
    test_spin();
    test_semaphore();
    0
}

entry!(main);