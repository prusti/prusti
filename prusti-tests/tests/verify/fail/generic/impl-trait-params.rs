use prusti_contracts::*;

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

// The synthetic type parameters of `a` and `b` have the same name. `b` is a
// `bool` at the call below, so `b.v() == 2`: the precondition, which is about
// `b`, not `a`, does not hold.
#[requires(b.v() == 1)]
#[trusted]
pub fn f(a: impl Val, b: impl Val) {}

pub fn g() {
    f(1u8, true); //~ ERROR: precondition might not hold
}
