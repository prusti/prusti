use prusti_contracts::*;

fn main() {
    let mut x = 3;
    let r_x = &mut x;
    let p_x = r_x as *mut i32;
    *r_x = *r_x + 1;
    unsafe { *p_x = *p_x + 1 };
    *r_x = *r_x + 1;
    prusti_assert!(x == 6);
}