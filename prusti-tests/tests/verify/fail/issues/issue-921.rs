// Regression test for prusti/prusti#921: the postconditions of the HashSet wrapper methods are
// used at the calls, so contradicting claims are reported.
use prusti_contracts::*;
use std::collections::HashSet;
use std::hash::Hash;

struct SetWrapper<T: Hash + Eq> {
    set: HashSet<T>,
}

impl<T: Hash + Eq> SetWrapper<T> {
    #[pure]
    #[trusted]
    pub fn contains(&self, value: &T) -> bool {
        self.set.contains(value)
    }

    #[trusted]
    #[ensures(self.contains(old(&value)))]
    pub fn insert(&mut self, value: T) {
        self.set.insert(value);
    }
}

fn not_contained_after_insert() {
    let mut set = SetWrapper { set: HashSet::new() };
    set.insert(1);
    prusti_assert!(!set.contains(&1)); //~ ERROR: the asserted expression might not hold
}
