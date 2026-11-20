// Coverage for `PtrToPtr` casts between raw pointers: a cast between pointers
// with the same metadata keeps the pointer, a cast from a wide to a thin
// pointer keeps only the address.

#![allow(ambiguous_wide_pointer_comparisons)]

use prusti_contracts::*;

fn thin_round_trip(p: *const u8) {
    assert!(p as *const i8 as *const u8 == p);
}

struct Holder {
    p: *const u8,
}

// A pointer read from a field rather than a local.
fn thin_round_trip_field(h: &Holder) {
    prusti_assert!(h.p as *const i8 as *const u8 == h.p);
}

fn wide_round_trip(a: *const [u8]) {
    assert!(a as *const [i8] as *const [u8] == a);
}

fn str_and_slice(s: *const str, b: *const [u8]) {
    if s as *const [u8] == b {
        assert!(s as *const u8 == b as *const u8);
    }
}

fn wide_eq_implies_thin_eq(a: *const [u8], b: *const [u8]) {
    if a == b {
        assert!(a as *const u8 == b as *const u8);
    }
}

fn mut_wide_eq_implies_thin_eq(a: *mut [u8], b: *mut [u8]) {
    if a == b {
        assert!(a as *mut u8 == b as *mut u8);
    }
}

// A thin pointer cast from the slice equals one cast from the array: both casts
// set the metadata to `()`.
fn slice_and_array(x: &[u8; 2]) {
    let s: &[u8] = x;
    assert!(s as *const [u8] as *const u8 == x as *const [u8; 2] as *const u8);
}

fn generic_wide_eq_implies_thin_eq<T: ?Sized>(a: *const T, b: *const T) {
    if a == b {
        assert!(a as *const u8 == b as *const u8);
    }
}

fn generic_sized<T>(p: *const T) {
    assert!(p as *const u8 as *const T == p);
}

// The cast in a body generic over `T: ?Sized` is encoded once; for a sized
// `T`, it keeps the metadata `()`.
#[pure]
fn to_thin<T: ?Sized>(p: *const T) -> *const u8 {
    p as *const u8
}

fn to_thin_sized(p: *const u8) {
    prusti_assert!(to_thin(p) == p);
}

fn to_thin_generic_sized<T>(p: *const T) {
    prusti_assert!(to_thin(p) == p as *const u8);
}

struct Tail<T: ?Sized> {
    x: u8,
    tail: T,
}

// A struct with an unsized tail has the metadata of its tail.
fn struct_tail(a: *const Tail<[u8]>) {
    assert!(a as *const [u8] as *const Tail<[u8]> == a);
}

fn generic_struct_tail<T: ?Sized>(a: *const Tail<T>) {
    assert!(a as *const T as *const Tail<T> == a);
}

#[pure]
fn tail_round_trip<T: ?Sized>(a: *const Tail<T>) -> *const Tail<T> {
    a as *const T as *const Tail<T>
}

fn tail_round_trip_slice(a: *const Tail<[u8]>) {
    prusti_assert!(tail_round_trip(a) == a);
}
