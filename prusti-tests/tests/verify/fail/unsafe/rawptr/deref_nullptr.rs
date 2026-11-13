use std::ptr;

pub fn main() {
    let p_x: *mut i32 = ptr::null::<i32>() as *mut i32;
    unsafe { *p_x = 5 }; //~ ERROR: the dereference of this raw pointer might not be permitted
}