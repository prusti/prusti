// Regression test for prusti/prusti#1366: values of associated types that are Copy.
use prusti_contracts::*;

trait Model {
    type State: Copy;
}

#[pure]
fn identity<M: Model>(x: M::State) -> M::State {
    x
}

#[pure]
fn twice<M: Model>(x: M::State) -> (M::State, M::State) {
    (x, x)
}

struct U;

impl Model for U {
    type State = i32;
}

fn instantiated(a: i32) {
    let r = identity::<U>(a);
    prusti_assert!(r == a);
    let (p, q) = twice::<U>(a);
    prusti_assert!(p == a && q == a);
}
