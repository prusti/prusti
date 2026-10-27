//@ compile-flags: -Passert_timeout=45000
// Review round 2: trait preconditions reading interior-mutable state cannot
// be established through the trait stub (incomplete, not unchecked); `acc`
// inside a quantifier; arrays of cells behind a struct field; the
// precondition of a drop contract.
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub trait T {
    #[requires(c.get() > 0)]
    #[ensures(result > 0)]
    fn m(&self, c: &Cell<i32>) -> i32;
}

pub fn generic<X: T>(x: &X) -> i32 {
    let c = Cell::new(-5);
    x.m(&c) //~ ERROR
}

#[trusted]
#[requires(forall(|i: usize| i == 0 ==> acc(p, Real::WRITE)))] //~ ERROR
fn touch_q(p: *mut i32) {}

pub struct S {
    pub cells: [Cell<i32>; 2],
}

pub fn clobber(s: &S) { //~ ERROR
    s.cells[0].set(9);
}

pub struct G {
    pub a: i32,
}

impl Drop for G {
    fn drop(&mut self) {}
}

#[extern_spec]
impl Drop for G {
    #[requires(self.a > 0)]
    fn drop(&mut self);
}

pub fn drop_g(x: G) {
    let _v = x.a;
} //~ ERROR
