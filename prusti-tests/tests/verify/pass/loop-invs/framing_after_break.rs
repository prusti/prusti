use prusti_contracts::*;

#[trusted]
fn random() -> i32 {
    unimplemented!()
}

fn test() {
    let x = 123;

    'myloop: while {
        if random() < x {
            break 'myloop;
        }

        random() < 345
    } {
        if random() < 456 {
            break;
        }

        let y = Box::new(x);
    }

    assert!(x == 123);
}

fn test2() {
    let mut x: i32;

    'myloop: while {
        x = 123;

        if random() < x {
            break 'myloop;
        }

        random() < 345
    } {
        body_invariant!(x == 123);
        if random() < 456 {
            break;
        }

        let y = Box::new(x);
    }

    assert!(x == 123);
}

fn test3() {
    let mut x: i32;

    'myloop: while {
        x = 123;

        if random() < x {
            break 'myloop;
        }

        random() < 345
    } {
        body_invariant!(x == 123);
        if random() < 456 {
            break;
        }

        x = 567;

        let y = Box::new(x);
    }

    assert!(x == 123);
}

// Places a loop only reads keep their value in and after the loop without an
// invariant about them.

struct S {
    a: i32,
    b: i32,
}

fn shared_ref(x: &i32) {
    let a = *x;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _b = *x;
        i += 1;
    }
    assert!(*x == a);
}

#[requires(u <= 5)]
fn at_most_five(u: usize) {}

#[requires(until <= 5)]
fn precondition_in_body(until: usize, x: &i32) {
    let a = *x;
    let mut i: usize = 0;
    while i < until {
        body_invariant!(i < until);
        at_most_five(until);
        prusti_assert!(*x == a);
        i += 1;
    }
}

#[requires(*y <= 5)]
fn read_at_most_five(y: &i32) {}

struct R<'a> {
    r: &'a mut i32,
}

#[requires(*x <= 5)]
#[ensures(*x == old(*x) + 1)]
fn mut_ref_read(x: &mut i32) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        read_at_most_five(x);
        i += 1;
    }
    *x += 1;
}

#[requires(*s.r <= 5)]
fn mut_ref_field_read(s: R) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        read_at_most_five(s.r);
        i += 1;
    }
}

#[requires(**x <= 5)]
fn nested_mut_ref_read(x: &mut &mut i32) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        read_at_most_five(*x);
        i += 1;
    }
}

#[trusted]
#[ensures(**r == 7)]
fn set_to_seven(r: &mut &mut i32) {}

// Passing on `&mut *x` may change `**x`, so it is not kept; `x` keeps its
// address, and the postcondition of the last call holds after the loop.
fn nested_mut_ref_passed_on(x: &mut &mut i32) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        set_to_seven(x);
        i += 1;
    }
    assert!(**x == 7);
}

#[requires(*x.r <= 5)]
fn mut_ref_field_behind_mut_ref_read(x: &mut R) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        read_at_most_five(x.r);
        i += 1;
    }
}

enum MaybeRef<'a> {
    Ref(&'a mut i32),
    Nothing,
}

#[requires(match &m { MaybeRef::Ref(r) => **r <= 5, MaybeRef::Nothing => true })]
fn mut_ref_in_enum_read(m: MaybeRef) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        if let MaybeRef::Ref(&mut ref v) = m {
            read_at_most_five(v);
        }
        i += 1;
    }
}

#[trusted]
fn look<T>(_x: &T) {}

// The loop uses nothing behind `s.r`, so its referent keeps its value.
#[requires(*s.r <= 5)]
fn mut_ref_field_read_as_whole(s: R) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        look(&s);
        i += 1;
    }
    read_at_most_five(s.r);
}

// The loop reads the reference and writes through it, so the reference keeps
// its address and the function can give back the permission to the referent.
fn mut_ref_read_and_written_through(x: &mut i32) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        look(&x);
        *x += 1;
        i += 1;
    }
}

fn mut_ref_field_read_and_written_through(s: R) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        look(&s);
        *s.r += 1;
        i += 1;
    }
}

struct Named {
    name: &'static str,
    n: u32,
}

// Assigning a `'static` reference behind `x` does not write `x` itself, so `x`
// keeps its address.
fn static_ref_written_behind_mut_ref(x: &mut Named) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        x.name = "a";
        i += 1;
    }
}

// The loop writes through `a` but not `a` itself, so the referent is at the
// same address after the loop.
fn mut_ref_written_through(a: &mut [u32; 8]) {
    let mut i = 0;
    while i < 8 {
        body_invariant!(i < 8);
        a[i] = 1;
        i += 1;
    }
}

fn owned_struct() {
    let s = S { a: 1, b: 2 };
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _t = s.a;
        i += 1;
    }
    assert!(s.a == 1 && s.b == 2);
}

fn generic_enum(o: Option<i32>) {
    let o0 = o;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        if let Some(v) = o {
            let _ = v;
        }
        i += 1;
    }
    assert!(matches!(o, Some(_)) == matches!(o0, Some(_)));
}

fn array_index() {
    let a = [1, 2, 3];
    let mut i = 0;
    while i < 3 {
        body_invariant!(i >= 0 && i < 3);
        let _t = a[i];
        i += 1;
    }
    assert!(a[0] == 1);
}

fn nested_loops(x: &S, s: S) -> S {
    let a = x.a;
    let sa = s.a;
    let sb = s.b;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let mut j = 0;
        while j < 10 {
            body_invariant!(j >= 0);
            let _t = x.a + s.b;
            j += 1;
        }
        i += 1;
    }
    assert!(x.a == a && s.a == sa && s.b == sb);
    s
}

fn nested_loops_inner(s: S) -> S {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let b = s.a;
        let mut j = 0;
        while j < 10 {
            body_invariant!(j >= 0);
            let _t = s.a;
            j += 1;
        }
        assert!(s.a == b);
        i += 1;
    }
    s
}

fn written_and_read_fields(mut s: S) -> S {
    let a = s.a;
    s.b = 0;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0 && s.b == i);
        let _t = s.a;
        s.b += 1;
        i += 1;
    }
    assert!(s.a == a);
    s
}
