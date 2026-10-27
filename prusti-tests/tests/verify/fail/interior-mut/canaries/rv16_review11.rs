//@ compile-flags: -Passert_timeout=45000
// Review round 11: pure trait methods called through their stub, snapshot
// equality in shallow trait contracts, pure functions returning references
// to interior-mutable values.
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub trait Tr {
    #[pure]
    #[ensures(result.get() == x)]
    fn mk(&self, x: i32) -> Cell<i32>;
}

pub fn stub_two<T: Tr>(t: &T) {
    let _a = t.mk(1);
    let _b = t.mk(2);
    assert!(false); //~ ERROR
}

pub fn stub_read_back<T: Tr>(t: &T) {
    let a = t.mk(1);
    assert!(a.get() == 1);
}

pub trait Same {
    #[requires(*a === *b)]
    #[ensures(result)]
    fn same(&self, a: &Cell<i32>, b: &Cell<i32>) -> bool;
}

pub fn snap_eq_pre<T: Same>(t: &T) {
    let c1 = Cell::new(1);
    let c2 = Cell::new(2);
    let r = t.same(&c1, &c2); //~ ERROR
    assert!(r);
}

#[pure]
pub fn idc(c: &Cell<i32>) -> &Cell<i32> {
    c
}

pub fn ref_results() {
    let c1 = Cell::new(1);
    let c2 = Cell::new(2);
    let r1 = idc(&c1);
    let r2 = idc(&c2);
    assert!(r1.get() == 1 && r2.get() == 2);
    r2.set(7);
    assert!(c2.get() == 7);
}

pub fn ref_results_ctrl() {
    let c1 = Cell::new(1);
    let c2 = Cell::new(2);
    let _r1 = idc(&c1);
    let _r2 = idc(&c2);
    assert!(false); //~ ERROR
}
