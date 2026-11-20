use prusti_contracts::*;

// Specs of functions with `impl Trait` arguments, whose synthetic type
// parameters all have the same name.
pub trait Val {
    #[pure]
    fn v(&self) -> u32;
}

#[refine_trait_spec]
impl Val for u8 {
    #[pure]
    fn v(&self) -> u32 {
        1
    }
}

#[refine_trait_spec]
impl Val for bool {
    #[pure]
    fn v(&self) -> u32 {
        2
    }
}

#[requires(a.v() == 1 && b.v() == 2)]
#[trusted]
pub fn f(a: impl Val, b: impl Val) {}

pub fn g() {
    f(1u8, true);
}

// In the spec items of an impl method, `Self` is replaced by the impl's type,
// also in the bound of an `impl Trait` argument, so the spec items' parameters
// cannot be matched to the method's by name. The trivial spec only gives the
// method a spec item.
pub trait Tr {
    fn m(x: impl Into<Self>)
    where
        Self: Sized;
}

pub struct Foo;

#[refine_trait_spec]
impl Tr for Foo {
    #[ensures(true)]
    fn m(x: impl Into<Self>) {}
}
