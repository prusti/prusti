//@ aux-build: my_trait_generic.rs
use my_trait_generic::MyTrait;
use prusti_contracts::*;

#[extern_spec(my_trait_generic)]
trait MyTrait<T> {
    #[trusted]
    #[ensures(true)]
    fn get_value(&self) -> T;
}

struct Impl;
#[refine_trait_spec]
impl MyTrait<i32> for Impl {
    #[ensures(result == 42)]
    fn get_value(&self) -> i32 {
        42
    }
}

fn main() {
    let s = Impl {};
    assert!(MyTrait::<i32>::get_value(&s) == 42 as i32);
}
