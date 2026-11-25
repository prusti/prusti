use prusti_contracts::*;

pub fn simple_loop() {
    let mut x = 0;
    while x < 100 {
        body_invariant!(x == 42); //~ERROR: loop invariant might not hold
        x += 1;
    }
}

// A place the loop only reads keeps its value, but nothing more is known.
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

fn read_only_ref_in_body(x: &i32) {
    let a = *x;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        prusti_assert!(*x == a);
        prusti_assert!(*x == a + 1); //~ERROR: might not hold
        i += 1;
    }
}

fn read_only_mut_ref_in_body(x: &mut i32) {
    let a = *x;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        prusti_assert!(*x == a);
        prusti_assert!(*x == a + 1); //~ERROR: might not hold
        i += 1;
    }
}

struct R<'a> {
    r: &'a mut i32,
}

#[requires(*s.r == 5)]
fn read_only_mut_ref_field_in_body(s: R) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        prusti_assert!(*s.r == 5);
        prusti_assert!(*s.r == 6); //~ERROR: might not hold
        i += 1;
    }
}

#[trusted]
fn look<T>(_x: &T) {}

#[requires(*s.r == 5)]
fn read_as_whole_in_body(s: R) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        look(&s);
        prusti_assert!(*s.r == 5);
        prusti_assert!(*s.r == 6); //~ERROR: might not hold
        i += 1;
    }
}

#[requires(**x == 5)]
fn read_only_nested_mut_ref_in_body(x: &mut &mut i32) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        prusti_assert!(**x == 5);
        prusti_assert!(**x == 6); //~ERROR: might not hold
        i += 1;
    }
}

#[requires(**x == 5)]
fn written_through_nested_mut_ref(x: &mut &mut i32) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        **x += 1;
        i += 1;
    }
    assert!(**x == 5); //~ERROR: might not hold
}

enum MaybeRef<'a> {
    Ref(&'a mut i32),
    Nothing,
}

#[requires(match &m { MaybeRef::Ref(r) => **r == 5, MaybeRef::Nothing => true })]
fn read_only_mut_ref_in_enum_in_body(m: MaybeRef) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        if let MaybeRef::Ref(&mut ref v) = m {
            prusti_assert!(*v == 5);
            prusti_assert!(*v == 6); //~ERROR: might not hold
        }
        i += 1;
    }
}

fn written_through_mut_ref(x: &mut i32) {
    let a = *x;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        *x += 1;
        i += 1;
    }
    assert!(*x == a); //~ERROR: might not hold
}

fn read_only_array() {
    let a = [1, 2, 3];
    let mut i = 0;
    while i < 3 {
        body_invariant!(i >= 0 && i < 3);
        let _t = a[i];
        i += 1;
    }
    assert!(a[0] == 1);
    assert!(a[1] == 1); //~ERROR: might not hold
}

struct S {
    a: i32,
    b: i32,
}

fn read_only_nested(s: S) -> S {
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
        assert!(s.b == b); //~ERROR: might not hold
        i += 1;
    }
    s
}

#[trusted]
fn random_bool() -> bool {
    unimplemented!()
}

fn test() {
    let mut y = None;
    let mut z = None;

    loop {
        let x = Box::new(5);
        if random_bool() {
            y = Some(x);
        } else {
            z = Some(x);
        }
        assert!(false); //~ERROR: might not hold
    }
}

#[trusted]
fn random() -> u32 {
    unimplemented!()
}

fn test1() {
    let mut tmp = Box::new(Box::new(random()));
    let tmp_ref = &tmp;
    let tmp_ref_mut = &mut tmp;

    while {
        let guard = random() < 55;

        let mut tmp = Box::new(Box::new(random()));
        let tmp_ref = &tmp;
        let tmp_ref_mut = &mut tmp;

        guard
    } {
        let mut tmp = Box::new(Box::new(random()));
        let tmp_ref = &tmp;
        let tmp_ref_mut = &mut tmp;
    }

    let mut tmp = Box::new(Box::new(random()));
    let tmp_ref = &tmp;
    let tmp_ref_mut = &mut tmp;
    assert!(false); //~ ERROR: might not hold
}

fn test2() {
    let mut tmp = Box::new(Box::new(random()));
    let tmp_ref = &tmp;
    let tmp_ref_mut = &mut tmp;

    while {
        let guard = random() < 55;

        let mut tmp = Box::new(Box::new(random()));
        let tmp_ref = &tmp;
        let tmp_ref_mut = &mut tmp;

        guard
    } {
        let mut tmp = Box::new(Box::new(random()));
        let tmp_ref = &tmp;
        let tmp_ref_mut = &mut tmp;
        assert!(false); //~ ERROR: might not hold
    }

    let mut tmp = Box::new(Box::new(random()));
    let tmp_ref = &tmp;
    let tmp_ref_mut = &mut tmp;
}
