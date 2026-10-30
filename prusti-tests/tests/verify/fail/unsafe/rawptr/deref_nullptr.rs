use prusti_contracts::*;
use std::ptr;

fn main() {
    let p_x: *mut i32 = ptr::null::<i32>() as *mut i32;
    unsafe { *p_x = 5 };
}