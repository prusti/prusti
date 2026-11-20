//! Quantifiers in contracts given by `extern_spec`, used at call sites.

use prusti_contracts::*;
use core::hash::{Hash, Hasher};
use std::cell::OnceCell;
use std::collections::hash_map::DefaultHasher;

#[extern_spec]
trait Hasher {
    #[trusted]
    #[ensures(forall(|i: u64| i == result ==> i <= 10))]
    // The body of this quantifier refers to the type of `self`.
    #[ensures(forall(|i: u64| self === self))]
    fn finish(&self) -> u64;
}

#[extern_spec]
trait Hash {
    #[trusted]
    // The body of this quantifier refers to a generic parameter of the method.
    #[ensures(forall(|x: &H| x === x))]
    fn hash<H: Hasher>(&self, state: &mut H);
}

#[extern_spec]
trait Iterator {
    #[trusted]
    // The type of `result` involves an associated type of the trait.
    #[ensures(forall(|i: u32| i == 0 ==> result === result))]
    fn next(&mut self) -> Option<Self::Item>;
}

// The parameter of the impl is named differently from that of the type
// definition (`OnceCell<T>`).
#[extern_spec]
impl<X> OnceCell<X> {
    #[trusted]
    #[ensures(forall(|x: &X| x === x))]
    fn get(&self) -> Option<&X>;
}

fn call_generic<T: Hasher>(h: &T) {
    let f = h.finish();
    assert!(f <= 10);
}

fn call_concrete(h: &DefaultHasher) {
    let f = h.finish();
    assert!(f <= 10);
}

fn call_hash<H: Hasher, T: Hash>(t: &T, h: &mut H) {
    t.hash(h);
}

fn call_next<I: Iterator>(i: &mut I) {
    let _n = i.next();
}

fn call_get<Y>(c: &OnceCell<Y>) {
    let _v = c.get();
}
