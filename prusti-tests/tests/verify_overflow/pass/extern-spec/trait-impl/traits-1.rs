//@ aux-build: max_static.rs
use max_static::{Max, TestStruct};
use prusti_contracts::*;

#[extern_spec]
impl Max for TestStruct {
    #[trusted]
    #[pure]
    #[ensures(result >= a && result >= b)]
    #[ensures(result == a || result == b)]
    fn max(a: i32, b: i32) -> i32;
}

fn main() {
    let x = TestStruct::max(1, 2);
    assert!(x == 2)
}
