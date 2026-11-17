use prusti_contracts::*;

struct S {
    f: i32
}

#[pure]
#[trusted]
fn pred(s: &S) -> bool {
    unimplemented!()
}

#[pure]
#[trusted]
#[ensures(result === *s)]
fn dup(s: &S) -> S {
    unimplemented!()
}

#[requires(pred(x))]
#[ensures(pred(&result))]
fn test(x: &S) -> S {
    dup(x)
}

#[pure]
#[trusted]
#[ensures(result >= 0)]
fn len(s: &S) -> i32 {
    unimplemented!()
}

/// The same call from executable code and from a specification must agree,
/// even though the reference passed in executable code has a real address
/// while the one built for the specification does not.
fn call_in_impure_code(x: &S) {
    let n = len(x);
    prusti_assert!(n == len(x));
}

#[pure]
fn len_twice(s: &S) -> i32 {
    len(s) + len(s)
}

/// A pure function with a body dummies the addresses of its own nested calls,
/// so a direct call of the callee must be dummied the same way.
fn nested_pure_call(x: &S) {
    let a = len_twice(x);
    let b = len(x);
    prusti_assert!(a == b + b);
}

/// A `#[pure(memory)]` function sees the addresses of its arguments, so it
/// can return a reference into one.
#[pure(memory)]
#[ensures(result as *const i32 == &s.f as *const i32)]
fn field(s: &S) -> &i32 {
    &s.f
}

/// Callers learn the referent and the address. On a reference argument, the
/// call in executable code and the same call in a specification agree.
fn memory_call(x: &S) {
    let r = field(x);
    prusti_assert!(*r == x.f);
    prusti_assert!(r as *const i32 == &x.f as *const i32);
    prusti_assert!(r === field(x));
}
