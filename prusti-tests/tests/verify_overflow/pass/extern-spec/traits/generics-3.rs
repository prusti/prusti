//@ aux-build: my_ord.rs
use my_ord::MyOrd;
use prusti_contracts::*;
use std::cmp::Ordering;

#[extern_spec(my_ord)]
trait MyOrd {
    #[trusted]
    #[pure]
    fn cmp(&self, other: &Self) -> Ordering;
}

#[pure]
fn compare<T: MyOrd>(a: &T, other: &T) -> Ordering {
    a.cmp(other)
}
