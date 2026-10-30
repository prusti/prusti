fn main() {
    let mut x = 3;
    let r_x = &mut x;
    let p_x = r_x as *mut i32;
    x = x + 1;
    unsafe { *p_x = *p_x + 1 }; //~ ERROR: the dereference of this raw pointer might not be permitted
}