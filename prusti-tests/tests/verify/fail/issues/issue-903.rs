// Regression test for prusti/prusti#903: calling a pure function returning a reference.
use prusti_contracts::*;

pub struct T {
    v: i32,
}

impl T {
    #[trusted]
    #[pure]
    #[ensures(result.v == self.v)]
    fn get(&self) -> &T {
        unimplemented!()
    }
}

pub fn wrong_postcondition(m: &T) {
    let r = m.get();
    prusti_assert!(r.v == m.v + 1); //~ ERROR: the asserted expression might not hold
}
