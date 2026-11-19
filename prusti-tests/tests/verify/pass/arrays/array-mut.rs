use prusti_contracts::*;

fn main() {
    let mut a = [1, 2, 3];
    a[1] = 23;

    assert!(a[0] == 1);
    assert!(a[1] == 23);
    assert!(a[2] == 3);
}

fn compound_assignment() {
    let mut a = [0; 3];
    a[0] += 1;
}

fn assignment_after_call() {
    println!("Hi");
    let mut t = [0];
    t[0] = 1;
}

fn looping() {
    let mut a = [0; 3];

    let mut i = 0;
    while i < 3 {
        body_invariant!(0 <= i && i < 3);
        a[i] = i;

        i += 1;
    }

    assert!(i == 3);
}

fn array_mut(mut a: [i32; 3]) {
    let a0 = a[0];
    let b = &mut a[0];
    assert!(*b == a0);
    *b = 42;
    // b expires here
    assert!(a[0] == 42);
}

// The other element keeps its reference after one is replaced.
#[ensures(*result[0] == 4)]
fn write_element<'a>(mut y: [&'a mut i32; 2], r: &'a mut i32) -> [&'a mut i32; 2] {
    y[1] = r;
    *y[0] = 4;
    y
}
