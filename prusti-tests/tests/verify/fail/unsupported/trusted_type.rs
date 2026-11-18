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
    t.x //~ERROR: cannot encode function body `read_pure`: Prusti does not look inside `T`
}

fn read(t: T) -> u32 {
    t.x //~ERROR: Prusti does not look inside `T`
}

fn read_ref(t: &T) -> u32 {
    t.x //~ERROR: Prusti does not look inside `T`
}

fn write(mut t: T) -> T {
    t.x = 5; //~ERROR: Prusti does not look inside `T`
    t
}

fn write_mut_ref(t: &mut T) {
    t.x = 5; //~ERROR: Prusti does not look inside `T`
}

#[pure]
fn write_pure(t: T) -> T {
    let mut t = t;
    t.x = 5; //~ERROR: cannot encode function body `write_pure`: Prusti does not look inside `T`
    t
}

#[pure]
fn construct_pure() -> T {
    T { x: 1 } //~ERROR: cannot encode function body `construct_pure`: Prusti does not look inside `T`
}

fn construct() -> T {
    T { x: 1 } //~ERROR: Prusti does not look inside `T`
}

#[trusted]
#[derive(Clone, Copy)]
enum E {
    A,
    B,
}

#[pure]
fn is_b(e: E) -> bool {
    matches!(e, E::B) //~ERROR: cannot encode function body `is_b`: Prusti does not look inside `E`
}

// `is_b` cannot be encoded, so `is_b(e)` is not known to be false and the
// precondition is not vacuous.
#[requires(is_b(e))]
fn requires_is_b(e: E) {
    assert!(false); //~ERROR: might not hold
}

fn match_impure(e: E) -> u32 {
    match e { //~ERROR: Prusti does not look inside `E`
        E::A => 1,
        E::B => 2,
    }
}

// The derived `eq` reads the discriminant through `discriminant_value`.
#[trusted]
#[derive(PartialEq)] //~ERROR: cannot encode function body `<P as std::cmp::PartialEq>::eq`: Prusti does not look inside `P`
enum P {
    A(u32),
    B,
}

#[pure]
fn get_pure(p: P) -> u32 {
    match p { //~ERROR: cannot encode function body `get_pure`: Prusti does not look inside `P`
        P::A(x) => x,
        P::B => 0,
    }
}

fn get(p: P) -> u32 {
    if let P::A(x) = p { x } else { 0 } //~ERROR: Prusti does not look inside `P`
}

fn construct_variant() -> P {
    P::A(3) //~ERROR: Prusti does not look inside `P`
}

#[pure]
fn construct_unit_variant_pure() -> P {
    P::B //~ERROR: cannot encode function body `construct_unit_variant_pure`: Prusti does not look inside `P`
}

#[pure]
fn discriminant_pure(p: &P) -> isize {
    core::intrinsics::discriminant_value(p) //~ERROR: cannot encode function body `discriminant_pure`: Prusti does not look inside `P`
}
