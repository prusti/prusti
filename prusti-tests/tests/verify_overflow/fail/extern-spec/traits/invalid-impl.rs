//@ aux-build: invalid_impl_aux.rs
use invalid_impl_aux::MyTrait;
use prusti_contracts::*;

#[extern_spec(invalid_impl_aux)]
trait MyTrait {
    #[trusted]
    #[ensures(result == 42)] //~ ERROR: postcondition might not hold.
    fn get_value(&self) -> i32;
}

struct Impl;
impl MyTrait for Impl {
    fn get_value(&self) -> i32 {
        43
    }
}

fn main() {
    let s = Impl {};
    assert!(MyTrait::get_value(&s) == 42);
}
