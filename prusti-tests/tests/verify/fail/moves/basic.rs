use prusti_contracts::*;

pub struct S2 {
    f: u32,
}

pub fn test4() -> S2 {
    let x = S2 {
        f: 8,
    };
    let y = x;
    assert!(y.f == 9);  //~ERROR: might not hold
    y
}

pub fn f(n: usize) {
    let x = ((String::new(), 1), (2, 5));
    let _y = x.0.0;
    let mut i = 0;
    loop {
        body_invariant!(x.1.1 == 6); //~ ERROR: loop invariant might not hold in the first loop iteration
        if i >= n {
            break;
        }
        i += 1;
    }
}

pub fn basic_partial_move() {
    let x = (String::new(), 5);
    let _y = x.0;
    prusti_assert!(x.1 == 6); //~ ERROR: the asserted expression might not hold
}

#[requires(x.1 == 5)]
pub fn box_partial_move(x: Box<(String, i32)>) {
    let _y = x.0;
    prusti_assert!(x.1 == 6); //~ ERROR: the asserted expression might not hold
}
