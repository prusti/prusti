//@ known-failure
//@ aux-build: my_trait_generic.rs
use my_trait_generic::MyTrait;
use prusti_contracts::*;

#[extern_spec(my_trait_generic)]
trait MyTrait<T> {
    // no equality constraints yet
    #[trusted]
    #[refine_spec(where T: SpecifiedGeneric, [
        ensures(result === T::my_trait__get_value()),
    ])]
    fn get_value(&self) -> T;
}

struct Impl;
impl MyTrait<i32> for Impl {
    fn get_value(&self) -> i32 {
        42
    }
}

fn main() {
    let s = Impl {};
    assert!(MyTrait::<i32>::get_value(&s) == 42);
    assert!(s.get_value() == 42);
}

// equality constraint workaround (in the general case, this would take an `&impl MyTrait<Self>` as arg)
trait SpecifiedGeneric {
    #[pure]
    fn my_trait__get_value() -> Self;
}

#[refine_trait_spec]
impl SpecifiedGeneric for i32 {
    #[pure]
    fn my_trait__get_value() -> Self {
        42
    }
}
