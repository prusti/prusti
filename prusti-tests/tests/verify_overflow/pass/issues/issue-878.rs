use prusti_contracts::*;
use std::cmp::{Ord, Ordering};

#[pure]
pub fn eq<T: Ord>(a: &T, b: &T) -> bool {
    match a.cmp(b) {
        Ordering::Equal => true,
        Ordering::Less => false,
        Ordering::Greater => false,
    }
}
