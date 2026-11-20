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
    prusti_assert!(to_thin(p) as *const T == p);
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

// The casts in bodies generic over `T: ?Sized` drop the metadata at unsized
// instantiations too, whose metadata types are not `()`: the result equals a
// thin cast after a cast to the tail, which keeps the metadata.
#[pure]
fn tail_thin<T: ?Sized>(a: *const Tail<T>) -> *const u8 {
    a as *const u8
}

fn tail_thin_slice(a: *const Tail<[u8]>) {
    prusti_assert!(tail_thin(a) == a as *const [u8] as *const u8);
}

fn tail_thin_nested(a: *const Tail<Tail<str>>) {
    prusti_assert!(tail_thin(a) == a as *const str as *const u8);
}

fn tail_thin_tuple(a: *const Tail<(u8, [u8])>) {
    prusti_assert!(tail_thin(a) == a as *const [u8] as *const u8);
}

// The same with the casts in the generic body instead.
#[pure]
fn tail_thin_via_tail<T: ?Sized>(a: *const Tail<T>) -> *const u8 {
    a as *const T as *const u8
}

fn tail_thin_via_tail_slice(a: *const Tail<[u8]>) {
    prusti_assert!(tail_thin_via_tail(a) == a as *const u8);
}

trait Tr {}

// `a as *const dyn Tr` would be an unsizing coercion, so a trait object tail is
// compared through the generic bodies only.
fn tail_thin_dyn(a: *const Tail<dyn Tr>) {
    prusti_assert!(tail_thin(a) == tail_thin_via_tail(a));
}

// A thin cast of a pointer read from a field.
struct H<T: ?Sized> {
    p: *const T,
}

#[pure]
fn field_thin<T: ?Sized>(h: &H<T>) -> *const u8 {
    h.p as *const u8
}

fn field_thin_slice(h: &H<[u8]>) {
    prusti_assert!(field_thin(h) == h.p as *const [i8] as *const u8);
}
