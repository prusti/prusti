//@ compile-flags: -Passert_timeout=45000
// Soundness canaries: a local holding an interior-mutable value is
// re-initialized (loops, scopes): every function must fail at `assert!(false)`.
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

fn loop_fresh_cell() {
    let mut i = 0;
    while i < 2 {
        body_invariant!(0 <= i && i <= 2);
        let c = Cell::new(i);
        assert!(c.get() == i);
        i += 1;
    }
    assert!(false); //~ ERROR
}

fn loop_fresh_cell_inner_false() {
    let mut i = 0;
    while i < 2 {
        body_invariant!(0 <= i && i <= 2);
        let c = Cell::new(i);
        c.set(i + 1);
        if i == 1 {
            assert!(false); //~ ERROR
        }
        i += 1;
    }
}
