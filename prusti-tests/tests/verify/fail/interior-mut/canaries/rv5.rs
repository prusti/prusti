//@ compile-flags: -Passert_timeout=45000
// Review round 1 follow-up: trait axioms see shallow values, so specs
// reading interior-mutable state stay in the methods' contracts only. A
// generic call goes through the trait stub, which states the trait's specs
// through those axioms only: the facts are not available after it
// (incomplete), and the contradictory shallow facts are gone (sound).
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub trait T {
    #[ensures(c.get() == 1)]
    fn a(&self, c: &Cell<i32>);
    #[ensures(c.get() == 2)]
    fn b(&self, c: &Cell<i32>);
}

pub fn generic<X: T>(x: &X, c: &Cell<i32>) {
    x.a(c);
    x.b(c);
    assert!(false); //~ ERROR
}
