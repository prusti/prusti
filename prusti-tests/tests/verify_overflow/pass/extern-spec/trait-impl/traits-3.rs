//@ aux-build: traits_3_aux.rs
use traits_3_aux::{Test, Trait};
use prusti_contracts::*;

#[extern_spec]
impl Trait for Test {
    #[trusted]
    #[ensures(result == 5)]
    fn foo(&mut self) -> i32;
}

fn main() {
    let mut t = Test {};
    assert!(t.foo() == 5)
}
