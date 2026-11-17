use prusti_contracts::*;

#[derive(PartialEq, PartialOrd)]
struct A {
    i: i32,
}

#[requires(_x < _y)]
fn test(_x: A, _y: A) {}
