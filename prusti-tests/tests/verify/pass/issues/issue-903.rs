// Regression test for prusti/prusti#903: calling a pure function returning a reference in a loop.
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

pub fn call_in_loop(m: &T) {
    loop {
        let _r = m.get();
    }
}

pub fn use_postcondition_in_loop(m: &T) {
    let mut i = 0;
    while i < 3 {
        body_invariant!(i < 3);
        let r = m.get();
        prusti_assert!(r.v == m.v);
        i += 1;
    }
}
