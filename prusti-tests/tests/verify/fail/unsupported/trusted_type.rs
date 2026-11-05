use prusti_contracts::*;

// A `#[trusted]` type is encoded as an opaque value, so its fields cannot be
// read or written, its values cannot be built from fields, and its
// discriminant cannot be read.
//
// A pure function is also encoded as a method. There, as in other non-pure
// code, an unsupported rvalue (building a value, reading a discriminant) only
// gives the generic "unsupported rvalue ... might be reached" error, and only
// where it may be reached.

#[trusted]
#[derive(Clone, Copy)]
struct T {
    x: u32,
}

#[pure]
fn read_pure(t: T) -> u32 {
    t.x //~ERROR: cannot encode function body `read_pure`: the fields of `T` cannot be used
    //~^ ERROR: cannot encode method body `read_pure`: the fields of `T` cannot be used
}

fn read(t: T) -> u32 {
    t.x //~ERROR: the fields of `T` cannot be used
}

fn read_ref(t: &T) -> u32 {
    t.x //~ERROR: the fields of `T` cannot be used
}

fn write(mut t: T) -> T {
    t.x = 5; //~ERROR: the fields of `T` cannot be used
    t
}

fn write_mut_ref(t: &mut T) {
    t.x = 5; //~ERROR: the fields of `T` cannot be used
}

#[pure]
fn construct_pure() -> T {
    T { x: 1 } //~ERROR: cannot encode function body `construct_pure`: the fields of `T` cannot be used
    //~^ ERROR: unsupported rvalue T
}

fn construct() -> T {
    T { x: 1 } //~ERROR: unsupported rvalue T
}

#[trusted]
#[derive(Clone, Copy)]
enum E {
    A,
    B,
}

#[pure]
fn is_b(e: E) -> bool {
    matches!(e, E::B) //~ERROR: cannot encode function body `is_b`: the discriminant of `E` cannot be used
    //~^ ERROR: unsupported rvalue discriminant
}

// `is_b` cannot be encoded, so `is_b(e)` is not known to be false and the
// precondition is not vacuous.
#[requires(is_b(e))]
fn requires_is_b(e: E) {
    assert!(false); //~ERROR: might not hold
}

fn match_impure(e: E) -> u32 {
    match e { //~ERROR: unsupported rvalue discriminant
        E::A => 1,
        E::B => 2,
    }
}

// The derived `eq` reads the discriminant through `discriminant_value`.
#[trusted]
#[derive(PartialEq)] //~ERROR: cannot encode function body `<P as std::cmp::PartialEq>::eq`: the discriminant of `P` cannot be used
//~^ ERROR: cannot encode method body `<P as std::cmp::PartialEq>::eq`: the discriminant of `P` cannot be used
enum P {
    A(u32),
    B,
}

// In the method body the discriminant read is an rvalue, so the field read is
// the first error reported.
#[pure]
fn get_pure(p: P) -> u32 {
    match p { //~ERROR: cannot encode function body `get_pure`: the discriminant of `P` cannot be used
        P::A(x) => x, //~ERROR: cannot encode method body `get_pure`: the fields of `P` cannot be used
        P::B => 0,
    }
}

fn get(p: P) -> u32 {
    if let P::A(x) = p { x } else { 0 } //~ERROR: the fields of `P` cannot be used
}

fn construct_variant() -> P {
    P::A(3) //~ERROR: unsupported rvalue P::A
}
