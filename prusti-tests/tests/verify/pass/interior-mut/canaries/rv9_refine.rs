//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;

trait T {
    #[ensures(result >= x)]
    fn f(&self, x: i32) -> i32;
}

struct S;

#[refine_trait_spec]
impl T for S {
    #[ensures(result == x)]
    fn f(&self, x: i32) -> i32 { x }
}
