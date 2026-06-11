use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
use spin::Mutex;
use x86_64::{
    instructions::port::Port,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame},
};

use super::{ack, consts::KEYBOARD_INTERRUPT_VEC};

lazy_static! {
    static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
        Mutex::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore
        ));
}

pub fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt[KEYBOARD_INTERRUPT_VEC].set_handler_fn(keyboard_handler);
}

pub extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::<u8>::new(0x60);
    let scancode = unsafe { port.read() };

    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(event) {
            match key {
                DecodedKey::Unicode(character) if character.is_ascii() => {
                    crate::drivers::input::push_key(character as u8);
                }
                DecodedKey::RawKey(_) | DecodedKey::Unicode(_) => {}
            }
        }
    }

    ack();
}
