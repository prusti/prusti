use prusti_contracts::*;

// The cases of `pass/generic/impure.rs` with generic fields instantiated with
// mutable references, with wrong assertions.

struct Foo<T> {
    f: T,
}

fn generic_field_body(foo: Foo<&mut i32>) {
    *foo.f = 1;
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

fn generic_field_construct() {
    let mut a = 1;
    let foo = Foo { f: &mut a };
    *foo.f = 2;
    assert!(a == 1); //~ ERROR: might not hold
}

fn generic_field_construct_false() {
    let mut a = 1;
    let foo = Foo { f: &mut a };
    *foo.f = 2;
    assert!(a == 2);
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

fn tuple_construct() {
    let mut a = 1;
    let mut b = 1;
    let t = (&mut a, &mut b);
    *t.0 = 2;
    *t.1 = 3;
    assert!(b == 2); //~ ERROR: might not hold
}

fn tuple_construct_false() {
    let mut a = 1;
    let mut b = 1;
    let t = (&mut a, &mut b);
    *t.0 = 2;
    *t.1 = 3;
    assert!(a == 2 && b == 3);
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

fn option_construct() {
    let mut a = 1;
    let o = Some(&mut a);
    if let Some(r) = o {
        *r = 2;
    }
    assert!(a == 1); //~ ERROR: might not hold
}

fn option_construct_false() {
    let mut a = 1;
    let o = Some(&mut a);
    if let Some(r) = o {
        *r = 2;
    }
    assert!(a == 2);
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

fn consume_foo(_foo: Foo<&mut i32>) {}

fn consume_option(_o: Option<&mut i32>) {}

fn generic_field_moved() {
    let mut a = 1;
    consume_foo(Foo { f: &mut a });
    let _b = a;
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

fn option_moved() {
    let mut a = 1;
    consume_option(Some(&mut a));
    let _b = a;
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}
