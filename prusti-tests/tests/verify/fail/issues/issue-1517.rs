use prusti_contracts::*;

// Regression test for prusti/prusti#1517.
struct S;

trait SFind {
    #[pure]
    fn is_s(&self) -> bool;
}

#[refine_trait_spec]
impl SFind for S {
    #[pure]
    fn is_s(&self) -> bool {
        true
    }
}

#[refine_trait_spec]
impl SFind for u32 {
    #[pure]
    fn is_s(&self) -> bool {
        false
    }
}

#[requires(T::is_s(&t))]
#[ensures(T::is_s(&t))]
#[ensures(!u32::is_s(&1u32))]
fn foo<T: SFind>(t: T) {}

#[requires(T::is_s(&t))]
#[ensures(u32::is_s(&1u32))] //~ ERROR: postcondition might not hold
fn foo_wrong<T: SFind>(t: T) {}

fn call_wrong() {
    foo::<S>(S);
    prusti_assert!(!S::is_s(&S)); //~ ERROR: might not hold
}
