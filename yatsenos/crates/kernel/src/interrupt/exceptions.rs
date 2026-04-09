
use log::info; 
use x86_64::VirtAddr;

use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

use crate::memory::*;

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt.divide_error.set_handler_fn(divide_error_handler);
    idt.double_fault
        .set_handler_fn(double_fault_handler)
        .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
    idt.page_fault
        .set_handler_fn(page_fault_handler)
        .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
    
    // TODO: you should handle more exceptions here
    // especially general protection fault (GPF)
    // see: https://wiki.osdev.org/Exceptions

    idt.general_protection_fault
    .set_handler_fn(general_protection_fault_handler)
    .set_stack_index(gdt::GENERAL_PROTECTION_FAULT_IST_INDEX);
    
    idt.breakpoint.set_handler_fn(breakpoint_handler);

    idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
}

pub extern "x86-interrupt" fn divide_error_handler(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: DIVIDE ERROR\n\n{:#?}", stack_frame);
}

pub extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
     panic!(
        "EXCEPTION: DOUBLE FAULT, ERROR_CODE: 0x{:016x}\n\n{:#?}",
        error_code, stack_frame
    );
}

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    err_code: PageFaultErrorCode,
) {
    panic!(
        "EXCEPTION: PAGE FAULT, ERROR_CODE: {:?}\n\nTrying to access: {:#x}\nStack_Frame: {:#?}",
        err_code,
        Cr2::read().unwrap_or(VirtAddr::new_truncate(0xdeadbeef)),
        stack_frame
    );
}

fn parse_gp_error(code: u64) -> &'static str{
    if code == 0{
        "Null Selector or General Violation"
    }else if code & 0b1 != 0{
        "External Event (Hardware)"
    }else {
        match  (code >>1)& 0b11 {
            0b00 => "GDT Violation",
            0b01 => "IDT Violation",
            0b10 => "LDT Violation",
            0b11 => "IDT Violation (with TI set)",
            _ => "Unknown Segment Violation",
        }
    }
}

pub extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    err_code: u64,
){
    panic!(
        "EXCEPTION: GENERAL_PROTECTION_FAULT,ERROR_CODE: {:#x}\n\nDescription: {}\n\nStack_Frame: {:#?}",
        err_code,
        parse_gp_error(err_code),
        stack_frame
    );
}

pub extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame){
        info!("EXCEPTION: BREAKPOINT\n\nStack_Frame: {:#?}",stack_frame)  
    }

pub extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame){
    panic!(
        "EXCEPTION: INVALID_OPCODE\n\nStack_Frame: {:#?}",
        stack_frame
    );
}