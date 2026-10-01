//@ compile-flags: -Passert_timeout=45000
// The interior-mutable objects of an enum are those of its active variant,
// also for enums with more than two variants.
use std::cell::Cell;

pub enum E {
    A(Cell<i32>),
    B(u8),
    C(Cell<i32>, Cell<i32>),
}

fn first_variant(e: &E) {
    if let E::A(c) = e {
        c.set(1);
        assert!(c.get() == 1);
    }
}

fn last_variant(e: &E) {
    if let E::C(x, _) = e {
        x.set(3);
        assert!(x.get() == 3);
    }
}
