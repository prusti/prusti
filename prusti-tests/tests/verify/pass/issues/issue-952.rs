// Regression test for prusti/prusti#952: `Option` holding a reference.
use prusti_contracts::*;
use std::collections::HashMap;

fn hash_map_get_unwrap(h: HashMap<u64, u64>, i: &u64) {
    let _a = h.get(i).unwrap();
}

fn option_ref_value(x: usize) {
    let o = Some(&x);
    match o {
        Some(a) => prusti_assert!(*a == x),
        None => prusti_assert!(false),
    }
}
