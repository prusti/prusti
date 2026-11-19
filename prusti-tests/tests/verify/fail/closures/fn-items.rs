use prusti_contracts::*;

// A fn item used as a value can be called through the stubs of its built-in
// `Fn*` impls, which neither take the targets of mutable references among its
// parameters nor establish its precondition. So such a fn item is rejected.

#[ensures(*x == 1)]
fn set_to_one(x: &mut i32) { //~ERROR: the function needs the targets of mutable references
    *x = 1;
}

fn app_mut<F: FnMut(&mut i32)>(mut f: F) -> i32 {
    let mut x = 0;
    f(&mut x);
    x
}

fn uses_set_to_one() {
    let _ = app_mut(set_to_one);
    // Through `impl<F: Fn> FnMut for &F`.
    let _ = app_mut(&set_to_one);
}

#[requires(x > 0)] //~ERROR: the function's precondition may be stronger than the trait method's
#[ensures(result == x - 1)]
fn dec(x: u32) -> u32 {
    x - 1
}

fn app<F: Fn(u32) -> u32>(f: F) -> u32 {
    f(0)
}

fn uses_dec() {
    let _ = app(dec);
}

// A fn item that only takes and returns values, called through `Fn`.
#[ensures(result == x + 1)]
fn inc(x: u32) -> u32 {
    x + 1
}

fn uses_inc() {
    let _ = app(inc);
    let f = inc;
    let _ = f(1);
}

// Direct calls do not use the `Fn*` impls, so a function only called
// directly is not checked.
#[requires(*x > 0)]
#[ensures(*x == old(*x) - 1)]
fn decrement(x: &mut i32) {
    *x -= 1;
}

fn direct_calls() {
    let mut y = 1;
    decrement(&mut y);
    assert!(y == 0);
    set_to_one(&mut y);
    assert!(y == 1);
    assert!(dec(1) == 0);
}

// A trait method used as a value is checked itself; the impls are checked
// against it.
trait Set {
    fn set(x: &mut i32); //~ERROR: the function needs the targets of mutable references
}

struct S;

impl Set for S {
    fn set(x: &mut i32) {
        *x = 1;
    }
}

fn uses_trait_method() {
    let _ = app_mut(<S as Set>::set);
}

// A constructor takes values and has no specification.
struct W(u32);

fn uses_constructor() -> W {
    let f = W;
    f(1)
}

// A function whose specification uses its own fn item as a value is accepted:
// its check does not need that specification while it is being encoded.
#[ensures({ let _f = id; result == x })]
fn id(x: u32) -> u32 {
    x
}

// An unsafe fn item has no `Fn*` impls, so it is not checked, despite its
// precondition.
#[requires(x > 0)]
unsafe fn unsafe_dec(x: u32) -> u32 {
    x - 1
}

fn uses_unsafe_fn() -> u32 {
    let f = unsafe_dec;
    unsafe { f(1) }
}
