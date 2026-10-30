use prusti_contracts::*;

// Simple specifications without old expressions

#[requires(x == 42)]
#[ensures(result == 42)]
fn test_unsigned_int(x: u32) -> u32 {
    assert!(x == 42);
    x
}

#[requires(x == -42)]
#[ensures(result == -42)]
fn test_signed_int(x: i32) -> i32 {
    assert!(x == -42);
    x
}

#[requires(x)]
#[ensures(result == true)]
fn test_bool(x: bool) -> bool {
    assert!(x);
    x
}

#[ensures(result == old(x))]
fn identity(x: i32) -> i32 {
    x
}

// Specification of box dereferentiation

#[ensures(result == old(*my_box))]
fn foo(my_box: Box<i32>) -> i32 {
    *my_box
}

// Encoding of fields in specifications

struct S {
    a: i32,
    b: i32
}

#[requires(x.0 == 123 && x.1 == 42)]
#[ensures(result.0 == 42 && result.1 == 123)]
fn test_tuple_field(x: (i32, i32)) -> (i32, i32) {
    (x.1, x.0)
}

#[requires(x.a == 123 && x.b == 42)]
#[ensures(result.a == 42 && result.b == 123)]
fn test_struct_field(x: S) -> S {
    S {
        a: x.b,
        b: x.a
    }
}
