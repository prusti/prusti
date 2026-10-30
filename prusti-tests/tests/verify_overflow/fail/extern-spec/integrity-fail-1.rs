//@ aux-build: max_trait_only.rs
// The extern spec below introduces a specification for TestStruct::max
// but not for Max::max (implemented on TestStruct)
// We do not consider the extern spec TestStruct::max for calls to Max::max

use max_trait_only::{Max, TestStruct};
use prusti_contracts::*;

#[extern_spec]
impl TestStruct {
    #[trusted]
    #[pure]
    #[ensures(result >= a && result >= b)]
    #[ensures(result == a || result == b)]
    fn max(a: i32, b: i32) -> i32; //~ ERROR: Invalid external specification
}

fn main() {
    let x = TestStruct::max(1, 2);
    assert!(x == 2)
}
