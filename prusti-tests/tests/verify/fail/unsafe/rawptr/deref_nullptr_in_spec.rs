use prusti_contracts::*;
use std::ptr;

fn main() {
    let p_x: *mut i32 = ptr::null::<i32>() as *mut i32;
    prusti_assert!(unsafe { *p_x == 0 } ); //~ ERROR the dereference of this raw pointer might not be permitted
}