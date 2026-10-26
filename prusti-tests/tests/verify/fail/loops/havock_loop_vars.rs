use prusti_contracts::*;

#[trusted]
fn random() -> bool {
    unimplemented!()
}

fn test() {
    let mut y = None;
    let mut z = None;

    loop {
        let x = Box::new(5);
        if random() {
            y = Some(x);
        } else {
            z = Some(x);
        }
        assert!(false); //~ERROR: might not hold
    }
}

struct S {
    a: i32,
    b: i32,
}

// A place the loop only reads keeps its value, but nothing more is known.
fn read_only(x: &i32) -> S {
    let s = S { a: 1, b: 2 };
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _t = *x + s.a;
        i += 1;
    }
    assert!(s.a == 1);
    assert!(s.b == 3); //~ERROR: might not hold
    s
}

fn read_only_ref(x: &i32) {
    let a = *x;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _t = *x;
        i += 1;
    }
    assert!(*x == a + 1); //~ERROR: might not hold
}

fn main() {}
