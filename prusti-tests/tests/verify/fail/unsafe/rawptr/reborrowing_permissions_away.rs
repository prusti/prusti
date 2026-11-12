pub fn main() {
    let mut x = 0;
    let r_x = &mut x;
    let p_x = r_x as *mut i32;
    let r_x_2 = &mut (*r_x);
    unsafe {*p_x = 5;}
    *r_x_2 = 5;
}