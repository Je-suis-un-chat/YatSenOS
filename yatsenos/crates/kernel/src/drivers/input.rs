use alloc::string::String;
use crossbeam_queue::ArrayQueue;

use crate::serial;

pub type Key = u8;

lazy_static!{
    static ref INPUT_BUF:ArrayQueue<Key> = ArrayQueue::new(128);
}

#[inline]
pub fn push_key(key:Key){
    if INPUT_BUF.push(key).is_err(){
        warn!("Input buffer is full.Dropping key '{:?}'",key);
    }
}

#[inline]
pub fn try_pop_key()->Option<Key>{
    INPUT_BUF.pop()
}

pub fn pop_key()->Key{
    loop{
        if let Some(key) = try_pop_key(){
            return key;
        }

        core::hint::spin_loop();
    }
}

pub fn get_line()->String{
    let mut s = String::with_capacity(128);

    loop{
        let key = pop_key();
        match key {
            b'\r' | b'\n' =>{
                if let Some(mut serial) = crate::serial::get_serial(){
                    serial.send(b'\n');
                } 
            return s;           }
            0x08 | 0x7F => {
                if !s.is_empty(){
                    s.pop();

                    if let Some(mut serial) = crate::serial::get_serial(){
                        serial.send(0x08);
                        serial.send(b' ');
                        serial.send(0x08);
                    }
                }
            }
            _ => {
                s.push(key as char);
                if let Some(mut serial) = crate::serial::get_serial(){
                    serial.send(key);
                }
            }
        }
    }
}