// A cast to a thin pointer drops the metadata, so wide pointers with the same
// address but different metadata have equal thin casts.

#![allow(ambiguous_wide_pointer_comparisons)]

use prusti_contracts::*;

fn thin_eq_implies_wide_eq(a: *const [u8], b: *const [u8]) {
    if a as *const u8 == b as *const u8 {
        assert!(a == b); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
    }
}

fn wide_ne_implies_thin_ne(a: *const [u8], b: *const [u8]) {
    if a != b {
        assert!(a as *const u8 != b as *const u8); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
    }
}

fn mut_wide_ne_implies_thin_ne(a: *mut [u8], b: *mut [u8]) {
    if a != b {
        assert!(a as *mut u8 != b as *mut u8); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
    }
}

fn generic_wide_ne_implies_thin_ne<T: ?Sized>(a: *const T, b: *const T) {
    if a != b {
        assert!(a as *const u8 != b as *const u8); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
    }
}

struct Tail<T: ?Sized> {
    x: u8,
    tail: T,
}

#[pure]
fn tail_thin<T: ?Sized>(a: *const Tail<T>) -> *const u8 {
    a as *const u8
}

// Negations of the pass tests' assertions on thin casts in a generic body at
// unsized instantiations. They would verify if the metadata facts the casts
// rely on were contradictory.
fn tail_thin_slice(a: *const Tail<[u8]>) {
    prusti_assert!(tail_thin(a) != a as *const u8); //~ERROR: the asserted expression might not hold
}

trait Tr {}

fn tail_thin_dyn(a: *const Tail<dyn Tr>) {
    prusti_assert!(tail_thin(a) != a as *const u8); //~ERROR: the asserted expression might not hold
}

fn tail_thin_nested(a: *const Tail<Tail<str>>) {
    prusti_assert!(tail_thin(a) != a as *const u8); //~ERROR: the asserted expression might not hold
}

fn tail_thin_tuple(a: *const Tail<(u8, [u8])>) {
    prusti_assert!(tail_thin(a) != a as *const u8); //~ERROR: the asserted expression might not hold
}
