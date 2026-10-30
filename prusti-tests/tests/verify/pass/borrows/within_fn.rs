fn ints() {
    let x = 42;
    let y = &x;
    assert!(*y == 42);
    assert!(x == 42);
}

fn bools() {
    let x = true;
    let y = &x;
    assert!(*y == true);
    assert!(x == true);
}

fn foo(_v: &mut [&mut i32]) {}

fn consume() {
    let mut x = 4_i32;
    let mut y = [&mut x];
    foo(&mut y);
}
