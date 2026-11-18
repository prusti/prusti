use prusti_contracts::*;

// Regression test for prusti/prusti#619: derived impls must be accepted.
#[derive(Debug)]
pub struct Foo {
    pub bar: i32,
}

#[derive(Debug)]
pub enum E {
    Bar,
    Baz,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct P {
    pub x: i32,
}

pub fn derived_eq() {
    let p = P { x: 1 };
    let q = P { x: 2 };
    prusti_assert!(p != q);
    prusti_assert!(p == P { x: 1 });
}
