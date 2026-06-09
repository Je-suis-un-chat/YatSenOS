#![no_std]
#![no_main]

extern crate lib;

use lib::*;

const N: usize = 5;
const ROUNDS: usize = 8;

// 0: 解决死锁版本
// 1: 故意构造死锁版本
// 2: 尝试构造饥饿现象
const MODE: usize = 0;

static CHOPSTICK: [Semaphore; N] = semaphore_array![
    0x2000, 0x2001, 0x2002, 0x2003, 0x2004
];

// 最多允许 4 个哲学家同时尝试拿筷子，破坏“环路等待”
static ROOM: Semaphore = Semaphore::new(0x2010);

fn main() -> isize {
    for i in 0..N {
        assert!(CHOPSTICK[i].init(1));
    }
    assert!(ROOM.init(N - 1));

    println!("dining philosophers start, mode = {}", MODE);

    let mut pids = [0u16; N];

    for id in 0..N {
        let pid = sys_fork();

        if pid == 0 {
            philosopher(id);
            sys_exit(0);
        }

        pids[id] = pid;
    }

    for pid in pids {
        sys_wait_pid(pid);
    }

    for i in 0..N {
        assert!(CHOPSTICK[i].remove());
    }
    assert!(ROOM.remove());

    println!("dining philosophers finished");
    0
}

fn philosopher(id: usize) {
    for round in 0..ROUNDS {
        think(id, round);
        take_chopsticks(id, round);
        eat(id, round);
        put_chopsticks(id);
    }

    println!("[P{} pid {}] done", id, sys_get_pid());
}

fn think(id: usize, round: usize) {
    let delay = random_delay(id, round, 0);

    if MODE == 2 && id == 4 {
        // 让 P4 思考更久，比较容易观察到“饥饿/机会少”
        long_delay(delay + 8);
    } else {
        long_delay(delay);
    }

    println!("[P{} pid {}] thinking round {}", id, sys_get_pid(), round);
}

fn eat(id: usize, round: usize) {
    let delay = random_delay(id, round, 1);

    println!(
        "[P{} pid {}] eating round {}, delay = {}",
        id,
        sys_get_pid(),
        round,
        delay
    );

    long_delay(delay);
}

fn take_chopsticks(id: usize, round: usize) {
    let left = id;
    let right = (id + 1) % N;

    if MODE == 1 {
        // 故意死锁：所有人都先拿左筷子，再等一会儿拿右筷子。
        CHOPSTICK[left].wait();
        println!("[P{}] got left chopstick {}", id, left);

        long_delay(12);

        CHOPSTICK[right].wait();
        println!("[P{}] got right chopstick {}", id, right);
        return;
    }

    // 解决方案：进入“餐厅”的人数最多为 4，避免 5 人各拿一根筷子后互等。
    ROOM.wait();

    if id % 2 == 0 {
        CHOPSTICK[left].wait();
        println!("[P{}] got left chopstick {}", id, left);

        long_delay(random_delay(id, round, 2));

        CHOPSTICK[right].wait();
        println!("[P{}] got right chopstick {}", id, right);
    } else {
        CHOPSTICK[right].wait();
        println!("[P{}] got right chopstick {}", id, right);

        long_delay(random_delay(id, round, 2));

        CHOPSTICK[left].wait();
        println!("[P{}] got left chopstick {}", id, left);
    }
}

fn put_chopsticks(id: usize) {
    let left = id;
    let right = (id + 1) % N;

    CHOPSTICK[left].signal();
    CHOPSTICK[right].signal();

    if MODE != 1 {
        ROOM.signal();
    }

    println!("[P{}] put chopsticks {} and {}", id, left, right);
}

fn random_delay(id: usize, round: usize, salt: usize) -> usize {
    let mut x = sys_get_pid() as usize;
    x ^= id * 1103515245;
    x ^= round * 12345;
    x ^= salt * 2654435761;

    // 简单 LCG，够用来制造不同延迟
    x = x.wrapping_mul(1664525).wrapping_add(1013904223);

    x % 10 + 1
}

#[inline(never)]
fn long_delay(times: usize) {
    for _ in 0..times {
        for _ in 0..0x1000 {
            core::hint::spin_loop();
        }
    }
}

entry!(main);