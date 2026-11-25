//@ known-failure

// `x.f.f` is the only leaf of `x`, so moving it out leaves no part of `x`
// with a value. The PCG then collapses `x` and expands it again for the
// assignment, which the encoding does not support.

struct T1 {
    f: u32,
    g: u32,
    h: u32,
}

struct S1 {
    f: T1,
}

struct S2 {
    f: S1,
}

fn _test4(b: bool, mut x: S2, y: T1) {
    if b {
        let z = x.f.f;
    }
    x.f.f = y;
}
