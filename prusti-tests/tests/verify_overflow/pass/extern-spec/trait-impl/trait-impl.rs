//@ known-failure
//@ aux-build: trait_impl_aux.rs
use trait_impl_aux::{Incrementable, Number};
use prusti_contracts::*;

#[extern_spec]
impl Incrementable for Number {
    #[trusted]
    #[ensures( self.0 == old(self.0) + 1 )]
    fn increment(&mut self);
}

fn main() {
    let mut num = Number { 0: 0 };
    num.increment();
    assert!(num.0 == 1);
    num.increment();
    assert!(num.0 == 2);
}