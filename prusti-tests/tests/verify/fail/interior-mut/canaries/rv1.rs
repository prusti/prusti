//@ compile-flags: -Passert_timeout=45000
// Review round 1, updated for round 9: `#[pure]` functions see deep values
// (and results with interior-mutable contents are tied deeply).
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

#[pure]
fn read(c: &Cell<i32>) -> i32 {
    c.get()
}

#[pure]
#[trusted]
#[ensures(result == c.get())]
fn read_trusted(c: &Cell<i32>) -> i32 {
    c.get()
}

// A spec reading only the result's contents stays in the method's contract
// (and is not a fact about a shared constant in `f_`).
#[pure]
#[trusted]
#[ensures(result.get() == 5)]
fn mk5() -> Cell<i32> {
    Cell::new(5)
}

#[pure]
#[trusted]
#[ensures(result.get() == 6)]
fn mk6() -> Cell<i32> {
    Cell::new(6)
}

fn client() {
    let a = mk5();
    let b = mk6();
    assert!(a.get() == 5 && b.get() == 6);
    assert!(read_trusted(&a) == 5);
    assert!(read(&b) == 6);
    a.set(7);
    assert!(read(&a) == 7);
    assert!(false); //~ ERROR
}
