//@ aux-build: duplicate_1_aux.rs
use duplicate_1_aux::TestStruct;
use prusti_contracts::*;

#[extern_spec]
impl TestStruct {
    #[trusted]
    #[pure]
    #[ensures(result >= a && result >= b)]
    #[ensures(result == a || result == b)]
    fn max(a: i32, b: i32) -> i32; //~ERROR: duplicate specification for `duplicate_1_aux::TestStruct::max`
}

fn main() {
    let x = TestStruct::max(1, 2);
    assert!(x == 2)
}
