// Regression test for prusti/prusti#921: a wrapper around HashSet with a pure method and
// postconditions on `&mut self` must not crash the encoder.
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

    #[trusted]
    #[ensures(!self.contains(old(&value)))]
    pub fn remove(&mut self, value: &T) -> bool {
        self.set.remove(value)
    }
}

fn test() {
    let mut set = SetWrapper { set: HashSet::new() };
    set.insert(1);
    prusti_assert!(set.contains(&1));
    let one = 1;
    set.remove(&one);
    prusti_assert!(!set.contains(&1));
}
