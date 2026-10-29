//@ known-failure
//@ aux-build: assoc_type_aux.rs
use assoc_type_aux::{Incrementable, Number};
use prusti_contracts::*;

#[extern_spec]
impl Incrementable for Number {
    #[trusted]
    #[ensures( self.0 == old(self.0) + 1 )]
    #[ensures( result == self.0 )]
    fn increment(&mut self) -> i32;
}

fn main() {
    let mut num = Number { 0: 0 };
    let res = num.increment();
    assert!(num.0 == 1);
    assert!(res == 1);
    let res = num.increment();
    assert!(num.0 == 2);
    assert!(res == 2);
}