//@ aux-build: partial_spec_aux.rs
use partial_spec_aux::{MyStruct, MyTrait};
use prusti_contracts::*;

#[extern_spec]
impl MyTrait for MyStruct {
    #[trusted]
    #[requires(x > 10)]
    #[ensures(result == x + 10)]
    fn foo(&self, x: i32) -> i32;
}

fn main(){
    let s = MyStruct {};
    let r = s.foo(20);
    assert!(r == 30);
}