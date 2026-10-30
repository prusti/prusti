//@ aux-build: shadowing_3_aux.rs
use shadowing_3_aux::{TestStruct, TestTrait};
use prusti_contracts::*;

#[extern_spec]
impl TestStruct {
    #[trusted]
    #[ensures(result == 42)]
    fn foo() -> i32;
}

#[extern_spec]
impl TestTrait for TestStruct {
    #[trusted]
    #[ensures(result == 24)]
    fn foo() -> i32;
}

fn main() {
    let x = TestStruct::foo();
    let y = <TestStruct as TestTrait>::foo();
    assert!(x == 42);
    assert!(y == 24);
}
