// Regression test for prusti/prusti#1517: pure trait methods called on concrete types in specs.
use prusti_contracts::*;

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

fn call() {
    foo::<S>(S);
    prusti_assert!(!u32::is_s(&1u32));
    prusti_assert!(S::is_s(&S));
}
