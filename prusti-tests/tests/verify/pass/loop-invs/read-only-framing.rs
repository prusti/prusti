use prusti_contracts::*;

// Places a loop only reads keep their value across the loop without an
// invariant about them.

struct S {
    a: i32,
    b: i32,
}

struct T {
    s: S,
    c: i32,
}

struct W<X> {
    v: X,
    n: u32,
}

#[trusted]
fn get(s: &S) -> i32 {
    s.a
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

fn shared_ref_fields(x: &S) {
    let a = x.a;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _t = x.a + x.b;
        i += 1;
    }
    assert!(x.a == a);
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
    let S { a, b: _ } = s;
    assert!(a == 1);
}

fn mut_ref_read_then_written(x: &mut S) {
    let a = x.a;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _t = x.a;
        i += 1;
    }
    assert!(x.a == a);
    x.b = 5;
    assert!(x.a == a && x.b == 5);
}

fn generic_fields<X: Copy>(w: W<X>) -> W<X> {
    let n = w.n;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _v = w.v;
        i += 1;
    }
    assert!(w.n == n);
    w
}

fn enum_downcast(o: Option<i32>) -> Option<i32> {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        if let Some(v) = o {
            let _ = v;
        }
        i += 1;
    }
    o
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
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let mut j = 0;
        while j < 10 {
            body_invariant!(j >= 0);
            let _t = x.a + s.b;
            j += 1;
        }
        assert!(x.a == a && s.a == sa);
        i += 1;
    }
    assert!(x.a == a && s.a == sa);
    s
}

fn borrowed_for_call(s: S) -> S {
    let a = s.a;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _t = get(&s);
        i += 1;
    }
    assert!(s.a == a);
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

fn break_with_nested_fields(x: &mut T) {
    let c = x.c;
    let mut i = 0;
    loop {
        body_invariant!(i >= 0);
        let a = x.s.a;
        if a > i {
            break;
        }
        if x.s.b > i {
            break;
        }
        i += 1;
    }
    assert!(x.c == c);
    x.s.a = 3;
    x.c = 1;
}

fn vec_len(v: &Vec<i32>) {
    let n = v.len();
    let mut i = 0;
    while i < v.len() {
        body_invariant!(i < v.len());
        let _t = v[i];
        i += 1;
    }
    assert!(v.len() == n);
}
