//@ known-failure
//@ compile-flags: -Passert_timeout=45000
// The soundness canary of `snd3.rs` that does not run yet: a local assigned in
// a loop gives an error without a position (a gap in the loop encoding of
// write-only locals, not specific to interior mutability).
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

fn reassign_in_loop() {
    let mut c = Cell::new(0);
    let mut i = 0;
    while i < 3 {
        body_invariant!(0 <= i && i <= 3);
        c = Cell::new(i);
        i += 1;
    }
    assert!(false); //~ ERROR
}
