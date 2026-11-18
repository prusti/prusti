// Regression test for prusti/prusti#474: pure functions can create references.
use prusti_contracts::*;

struct TwoPSet;

impl TwoPSet {
    #[pure]
    #[trusted]
    #[ensures(result == (*value == 3))]
    fn inner_contains(&self, value: &u64) -> bool {
        unimplemented!()
    }

    #[pure]
    fn contains(&self, val: u64) -> bool {
        self.inner_contains(&val)
    }
}

fn contains_three(s: &TwoPSet) {
    prusti_assert!(s.contains(3));
    prusti_assert!(!s.contains(4));
}
