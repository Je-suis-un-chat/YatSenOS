static mut COUNTER: usize = 0;

fn main() {
    let mut handles = vec![];

    for _ in 0..10 {
        handles.push(std::thread::spawn(|| {
            for _ in 0..1000 {
                unsafe {
                    COUNTER += 1;
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Result: {}", unsafe { COUNTER });
}

/*
❯ rustc test.rs
❯ for ((i=16;i>=0;i--)); do ./test; done
Result: 9863
Result: 10000
Result: 10000
Result: 10000
Result: 8670
Result: 10000
Result: 10000
Result: 10000
Result: 10000
Result: 10000
Result: 9994
Result: 9401
Result: 10000
Result: 9334
Result: 10000
Result: 9349
Result: 10000

*/