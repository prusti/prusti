//@ aux-build: trait_bounds_aux.rs
// ignore-test fails because of const generics
use trait_bounds_aux::Foo;
use prusti_contracts::*;

#[extern_spec]
impl<'a, T: PartialEq, const L: usize> Foo<'a, T, L> {
    #[trusted]
    #[pure]
    #[ensures(result == self.0)]
    pub fn bar(self) -> &'a [T; L];
}
