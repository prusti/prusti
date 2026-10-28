use prusti_contracts::*;

pub struct S<'a> {
    pub r: &'a mut i32,
}

// A pure function's argument has a deep snapshot, holding the value behind its
// mutable reference.
#[pure]
fn get_s(s: S<'_>) -> i32 {
    *s.r
}

// A tuple is encoded over its element types as type parameters, whose
// snapshots are shallow, so a value read out of it lacks the values behind its
// mutable references, also when it is copied.
#[pure]
fn f(t: (S<'_>, i32)) -> i32 { //~ ERROR: the result of pure function `f` might not match its definition
    get_s(t.0) //~ ERROR: passing `S<'_>` to a pure function from a place that does not hold the values behind its mutable references
}

#[requires(get_s(t.0) == 1)] //~ ERROR: passing `S<'_>` to a pure function from a place that does not hold the values behind its mutable references
fn in_spec(t: (S<'_>, i32)) {}

trait Tr {
    #[ensures({ let r = s.0; result == old(*r) })] //~ ERROR: dereference of the mutable reference `&mut i32` in pure code, whose snapshot does not hold the value behind it
    fn get<'a>(&self, s: (&'a mut i32, i32)) -> i32;
}

// Impure code reads the value from the heap.
fn impure(t: (S<'_>, i32)) {
    *t.0.r = 5;
    let v = get_s(t.0);
    prusti_assert!(v == 5);
}

fn client<X: Tr>(t: &X, x: &mut i32) {
    let _ = f((S { r: &mut *x }, 0));
    let _ = t.get((&mut *x, 0));
}

fn main() {}
