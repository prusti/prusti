//! A `#[refine_trait_spec]` implementation may strengthen the trait method's
//! specification. Here the impl keeps the method `#[pure]` (as required to
//! refine a pure trait method) and adds a postcondition, which callers can then
//! rely on.

use core::ops::Bound;
use prusti_contracts::*;

trait Trait {
    #[pure]
    fn foo(&self) -> i32;
}

struct Struct(i32);

#[refine_trait_spec]
impl Trait for Struct {
    #[pure]
    #[ensures(result == self.0)]
    fn foo(&self) -> i32 {
        self.0
    }
}

fn client(s: &Struct) {
    // Relies on the refined postcondition of `foo`.
    assert!(s.foo() == s.0);
}

/// A `#[pure(memory)]` trait method may return references into `self`, like
/// `RangeBounds::start_bound`. An implementation may keep that mode or refine
/// it to the stronger `#[pure]`.
trait Bounds {
    #[pure(memory)]
    fn start(&self) -> Bound<&usize>;
}

struct Lower {
    lo: usize,
}

#[refine_trait_spec]
impl Bounds for Lower {
    #[pure(memory)]
    fn start(&self) -> Bound<&usize> {
        Bound::Included(&self.lo)
    }
}

struct Full;

#[refine_trait_spec]
impl Bounds for Full {
    #[pure]
    fn start(&self) -> Bound<&usize> {
        Bound::Unbounded
    }
}

fn bounds_client<B: Bounds>(b: &B) {
    let s = b.start();
    prusti_assert!(s === b.start());
}

fn lower_client(r: &Lower) {
    if let Bound::Included(lo) = r.start() {
        prusti_assert!(*lo == r.lo);
    } else {
        unreachable!();
    }
}
