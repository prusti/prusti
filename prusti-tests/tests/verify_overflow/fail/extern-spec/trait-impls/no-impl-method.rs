// An extern spec of a trait method in an impl block must specify a method
// defined by an impl block.
use core::cmp::Ordering;
use prusti_contracts::*;

// `Clone` for tuples is provided by the compiler.
#[extern_spec]
impl Clone for (i32, bool) {
    #[trusted]
    #[ensures(true)]
    fn clone(&self) -> Self; //~ ERROR: Invalid external specification
}

#[extern_spec]
impl (i32, bool) {
    #[trusted]
    #[ensures(true)]
    fn clone(&self) -> Self; //~ ERROR: Invalid external specification
}

// `Ordering` uses the trait's default `ne`.
#[extern_spec]
impl PartialEq for Ordering {
    #[trusted]
    #[ensures(true)]
    fn ne(&self, other: &Self) -> bool; //~ ERROR: Invalid external specification
}

#[extern_spec]
impl Ordering {
    #[trusted]
    #[ensures(true)]
    fn ne(&self, other: &Self) -> bool; //~ ERROR: Invalid external specification
}

// The where-clause `T: Clone` provides the method.
#[extern_spec]
impl<T: Clone> Clone for T {
    #[trusted]
    #[ensures(true)]
    fn clone(&self) -> T; //~ ERROR: Invalid external specification
}
