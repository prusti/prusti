pub fn foo() {
    let mut x = 5;
    let r_x = &mut x;
    let p_x = r_x as *mut i32;

    unsafe {
        let r_x_2 = &mut (*r_x);
        let p_x_2 = r_x_2 as *mut i32;
        *p_x = 5; 
        *p_x_2 = 5;
    }
}

pub fn foo2() {
    let mut x = 5;
    let r_x = &mut x;
    let p_x = r_x as *mut i32;

    unsafe {
        let r_x_2 = &mut (*p_x);
        let p_x_2 = r_x_2 as *mut i32;
        *p_x = 5; 
        *p_x_2 = 5;
    }
}