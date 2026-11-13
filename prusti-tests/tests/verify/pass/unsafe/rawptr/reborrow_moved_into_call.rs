use prusti_contracts::*;

#[ensures(*a == 1)]
fn set(a: &mut i32) {
    *a = 1;
}

pub fn main() {
    let mut x = 0;
    let r_x = &mut x;
    // Implicit reborrow `&mut *r_x`, which is moved into the call. When it
    // expires after the call, its permission must be transferred back to
    // `*r_x` without reading the moved reference in the current state.
    set(r_x);
    let p_x = r_x as *mut i32;
    unsafe { *p_x = *p_x + 1 };
    prusti_assert!(x == 2);
}
