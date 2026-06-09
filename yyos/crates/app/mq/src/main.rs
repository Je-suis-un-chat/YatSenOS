#![no_std]
#![no_main]

extern crate lib;

use lib::*;

const PROCESS_COUNT: usize = 16;
const PRODUCER_COUNT: usize = PROCESS_COUNT / 2;
const CONSUMER_COUNT: usize = PROCESS_COUNT / 2;
const MSG_PER_PROCESS: usize = 10;
const MAX_CAPACITY: usize = 16;

static MUTEX: Semaphore = Semaphore::new(0x1001);
static EMPTY: Semaphore = Semaphore::new(0x1002);
static FULL: Semaphore = Semaphore::new(0x1003);

static mut QUEUE: [usize; MAX_CAPACITY] = [0; MAX_CAPACITY];
static mut HEAD: usize = 0;
static mut TAIL: usize = 0;
static mut COUNT: usize = 0;
static mut CAPACITY: usize = 0;

fn main() -> isize {
    run_test(1);
    run_test(4);
    run_test(8);
    run_test(16);

    0
}

fn run_test(capacity: usize) {
    reset_queue(capacity);

    assert!(MUTEX.init(1));
    assert!(EMPTY.init(capacity));
    assert!(FULL.init(0));

    println!("\n========== mq test capacity = {} ==========", capacity);

    let mut pids = [0u16; PROCESS_COUNT];

    for i in 0..PROCESS_COUNT {
        let pid = sys_fork();

        if pid == 0 {
            if i < PRODUCER_COUNT {
                producer(i);
            } else {
                consumer(i - PRODUCER_COUNT);
            }

            sys_exit(0);
        }

        pids[i] = pid;
    }

    println!("parent created {} processes", PROCESS_COUNT);
    sys_stat();

    for pid in pids {
        sys_wait_pid(pid);
    }

    let final_count = unsafe { COUNT };
    println!(
        "mq capacity {} finished, final queue count = {}",
        capacity,
        final_count
    );

    assert_eq!(final_count, 0);

    assert!(MUTEX.remove());
    assert!(EMPTY.remove());
    assert!(FULL.remove());
}

fn reset_queue(capacity: usize) {
    unsafe {
        HEAD = 0;
        TAIL = 0;
        COUNT = 0;
        CAPACITY = capacity;

        for i in 0..MAX_CAPACITY {
            QUEUE[i] = 0;
        }
    }
}

fn producer(id: usize) {
    for seq in 0..MSG_PER_PROCESS {
        note_if_full(id);

        EMPTY.wait();
        MUTEX.wait();

        let msg = make_message(id, seq);
        push_message(msg);

        let count = unsafe { COUNT };
        let capacity = unsafe { CAPACITY };

        println!(
            "[producer {} pid {}] push msg {}, count = {}/{}",
            id,
            sys_get_pid(),
            msg,
            count,
            capacity
        );

        MUTEX.signal();
        FULL.signal();

        delay();
    }
}

fn consumer(id: usize) {
    for _ in 0..MSG_PER_PROCESS {
        note_if_empty(id);

        FULL.wait();
        MUTEX.wait();

        let msg = pop_message();

        let count = unsafe { COUNT };
        let capacity = unsafe { CAPACITY };

        println!(
            "[consumer {} pid {}] pop msg {}, count = {}/{}",
            id,
            sys_get_pid(),
            msg,
            count,
            capacity
        );

        MUTEX.signal();
        EMPTY.signal();

        delay();
    }
}

fn make_message(producer_id: usize, seq: usize) -> usize {
    producer_id * 1000 + seq
}

fn push_message(msg: usize) {
    
    unsafe {
        assert!(COUNT < CAPACITY);

        QUEUE[TAIL] = msg;
        TAIL = (TAIL + 1) % CAPACITY;
        COUNT += 1;
    }
}

fn pop_message() -> usize {
    
    unsafe {
        assert!(COUNT > 0);

        let msg = QUEUE[HEAD];
        HEAD = (HEAD + 1) % CAPACITY;
        COUNT -= 1;

        msg
    }
}

fn note_if_full(id: usize) {
    MUTEX.wait();

    let (count, capacity) = unsafe { (COUNT, CAPACITY) };

    if count == capacity {
        println!(
            "[producer {} pid {}] queue full, waiting; count = {}/{}",
            id,
            sys_get_pid(),
            count,
            capacity
        );
    }

    MUTEX.signal();
}

fn note_if_empty(id: usize) {
    MUTEX.wait();

    let (count, capacity) = unsafe { (COUNT, CAPACITY) };

    if count == 0 {
        println!(
            "[consumer {} pid {}] queue empty, waiting; count = {}/{}",
            id,
            sys_get_pid(),
            count,
            capacity
        );
    }

    MUTEX.signal();
}

#[inline(never)]
fn delay() {
    for _ in 0..0x1000 {
        core::hint::spin_loop();
    }
}

entry!(main);