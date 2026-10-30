//@ aux-build: partial_spec_2_aux.rs
use partial_spec_2_aux::{Struct1, Struct2, Trait};
use prusti_contracts::*;

#[extern_spec]
impl Trait for Struct1 {
    #[trusted]
    #[ensures( result == 42 )]
    fn foo(&self) -> i32;
}

fn main() {
    let s1 = Struct1 {};
    assert!(s1.foo() == 42);

    let s2 = Struct2 {};
    assert!(s2.foo() == 43); //~ ERROR: might not hold
}