//@ compile-flags: -Passert_timeout=45000
// Review round 13: guards framed out by a loop's invariant are not re-read
// after calls in the loop.
use prusti_contracts::*;
use prusti_contracts::rc::Rc;

pub fn noop(x: i32) -> i32 { x }

pub fn unused_rc() {
    let r = Rc::new(1);
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        noop(i);
        i += 1;
    }
    let _y = Rc::strong_count(&r);
}

pub fn used_rc() {
    let r = Rc::new(1);
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _x = Rc::strong_count(&r);
        i += 1;
    }
}

pub fn ctrl() { assert!(false); } //~ ERROR
