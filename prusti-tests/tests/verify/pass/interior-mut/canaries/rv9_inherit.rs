//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;

pub trait T {
    #[ensures(result >= x)]
    fn f(&self, x: i32) -> i32;
}

pub struct S;

impl T for S {
    fn f(&self, x: i32) -> i32 { x }
}
