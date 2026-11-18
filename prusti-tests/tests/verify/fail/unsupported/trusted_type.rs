#![feature(core_intrinsics)]
use prusti_contracts::*;

// A `#[trusted]` type is encoded as an opaque value, so its fields cannot be
// read or written, its values cannot be built from fields, and its
// discriminant cannot be read.

#[trusted]
#[derive(Clone, Copy)]
struct T {
    x: u32,
}

#[pure]
fn read_pure(t: T) -> u32 {
    t.x //~ERROR: cannot encode function body `read_pure`: using the contents of `T` is not supported
}

fn read(t: T) -> u32 {
    t.x //~ERROR: using the contents of `T` is not supported
}

fn read_ref(t: &T) -> u32 {
    t.x //~ERROR: using the contents of `T` is not supported
}

fn write(mut t: T) -> T {
    t.x = 5; //~ERROR: using the contents of `T` is not supported
    t
}

fn write_mut_ref(t: &mut T) {
    t.x = 5; //~ERROR: using the contents of `T` is not supported
}

#[pure]
fn write_pure(t: T) -> T {
    let mut t = t;
    t.x = 5; //~ERROR: cannot encode function body `write_pure`: using the contents of `T` is not supported
    t
}

#[pure]
fn construct_pure() -> T {
    T { x: 1 } //~ERROR: cannot encode function body `construct_pure`: using the contents of `T` is not supported
}

fn construct() -> T {
    T { x: 1 } //~ERROR: using the contents of `T` is not supported
}

#[trusted]
#[derive(Clone, Copy)]
enum E {
    A,
    B,
}

#[pure]
fn is_b(e: E) -> bool {
    matches!(e, E::B) //~ERROR: cannot encode function body `is_b`: using the contents of `E` is not supported
}

// `is_b` cannot be encoded, so `is_b(e)` is not known to be false and the
// precondition is not vacuous.
#[requires(is_b(e))]
fn requires_is_b(e: E) {
    assert!(false); //~ERROR: might not hold
}

fn match_impure(e: E) -> u32 {
    match e { //~ERROR: using the contents of `E` is not supported
        E::A => 1,
        E::B => 2,
    }
}

// The derived `eq` reads the discriminant through `discriminant_value`.
#[trusted]
#[derive(PartialEq)] //~ERROR: cannot encode function body `<P as std::cmp::PartialEq>::eq`: using the contents of `P` is not supported
enum P {
    A(u32),
    B,
}

#[pure]
fn get_pure(p: P) -> u32 {
    match p { //~ERROR: cannot encode function body `get_pure`: using the contents of `P` is not supported
        P::A(x) => x,
        P::B => 0,
    }
}

fn get(p: P) -> u32 {
    if let P::A(x) = p { x } else { 0 } //~ERROR: using the contents of `P` is not supported
}

fn construct_variant() -> P {
    P::A(3) //~ERROR: using the contents of `P` is not supported
}

#[pure]
fn construct_unit_variant_pure() -> P {
    P::B //~ERROR: cannot encode function body `construct_unit_variant_pure`: using the contents of `P` is not supported
}

#[pure]
fn discriminant_pure(p: &P) -> isize {
    core::intrinsics::discriminant_value(p) //~ERROR: cannot encode function body `discriminant_pure`: using the contents of `P` is not supported
}
