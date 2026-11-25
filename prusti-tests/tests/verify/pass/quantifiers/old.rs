use prusti_contracts::*;
// The builtin under another name, which the specification parser leaves alone,
// to build a quantifier's closure as a `move` closure or apart from the
// quantifier.
use prusti_contracts::forall as quantify;

pub struct VecI32 {
    v: Vec<i32>,
}

impl VecI32 {
    #[trusted]
    #[pure]
    pub fn len(&self) -> usize {
        self.v.len()
    }

    #[trusted]
    #[pure]
    #[requires(index < self.len())]
    pub fn lookup(&self, index: usize) -> i32 {
        self.v[index]
    }
}

#[trusted]
#[requires(from <= until && until <= arr.len())]
#[ensures(arr.len() == old(arr.len()))]
#[ensures(forall(|i: usize, j: usize| (from <= i && i < j && j < until) ==> arr.lookup(i) <= arr.lookup(j),
                 triggers = [(arr.lookup(i), arr.lookup(j))]))]
#[ensures(forall(|i: usize| (i < from || (until <= i && i < arr.len())) ==> arr.lookup(i) == old(arr.lookup(i)),
                 triggers = [(arr.lookup(i),)]))]
fn sort_part(arr: &mut VecI32, from: usize, until: usize) {}

/// `old` inside a quantifier in the postcondition of a `&mut` function reads
/// the pre-state: the frame carries a value across the call.
#[requires(0 < arr.len())]
fn frame(arr: &mut VecI32) {
    let x = arr.lookup(0);
    let n = arr.len();
    sort_part(arr, 1, n);
    prusti_assert!(arr.lookup(0) == x);
}

/// The same, with `old` in the assertion: it reads the state at the
/// function's entry.
#[requires(0 < arr.len())]
fn frame_old(arr: &mut VecI32) {
    let n = arr.len();
    sort_part(arr, 1, n);
    prusti_assert!(arr.lookup(0) == old(arr.lookup(0)));
}

/// Two calls composed through the frame of the second: what the first sorted
/// is still sorted after the second.
#[requires(from <= mid && mid <= until && until <= arr.len())]
fn compose(arr: &mut VecI32, from: usize, mid: usize, until: usize) {
    sort_part(arr, mid, until);
    sort_part(arr, from, mid);
    prusti_assert!(forall(|i: usize, j: usize| (mid <= i && i < j && j < until) ==> arr.lookup(i) <= arr.lookup(j),
                          triggers = [(arr.lookup(i), arr.lookup(j))]));
}

/// `old` in an inner quantifier reads the pre-state too.
#[trusted]
#[ensures(arr.len() == old(arr.len()))]
#[ensures(forall(|i: usize| forall(|j: usize| (i < arr.len() && j == i) ==> arr.lookup(i) == old(arr.lookup(j)),
                                   triggers = [(arr.lookup(j),)]),
                 triggers = [(arr.lookup(i),)]))]
fn keep_nested(arr: &mut VecI32) {}

#[requires(0 < arr.len())]
fn nested_frame(arr: &mut VecI32) {
    let x = arr.lookup(0);
    keep_nested(arr);
    prusti_assert!(arr.lookup(0) == x);
}

/// `old` around nested quantifiers reads both bodies in the pre-state. The
/// outer trigger is `i < arr.len()` rather than `arr.lookup(i)`, with which
/// Z3 fails to prove `old_around_quantifier` (prusti/prusti#1687).
#[trusted]
#[requires(0 < arr.len())]
#[ensures(arr.len() == old(arr.len()))]
#[ensures(old(forall(|i: usize| forall(|j: usize| (i < arr.len() && j == i) ==> arr.lookup(i) == 5 && arr.lookup(j) == 5,
                                       triggers = [(arr.lookup(j),)]),
                     triggers = [(i < arr.len(),)])) ==> arr.lookup(0) == 7)]
fn set_if_all_five(arr: &mut VecI32) {}

#[requires(0 < arr.len())]
fn old_around_quantifier(arr: &mut VecI32) {
    prusti_assume!(forall(|i: usize| i < arr.len() ==> arr.lookup(i) == 5,
                          triggers = [(arr.lookup(i),)]));
    set_if_all_five(arr);
    prusti_assert!(arr.lookup(0) == 7);
}

/// A quantified `old` postcondition proven, not just assumed.
#[ensures(arr.len() == old(arr.len()))]
#[ensures(forall(|i: usize| i < arr.len() ==> arr.lookup(i) == old(arr.lookup(i)),
                 triggers = [(arr.lookup(i),)]))]
fn unchanged(arr: &mut VecI32) {}

#[pure]
#[trusted]
#[ensures(result)]
fn tick(_i: usize) -> bool {
    true
}

/// `before_expiry` inside a quantified pledge reads the target of the
/// borrow just before the borrow expires.
#[trusted]
#[after_expiry(forall(|i: usize| tick(i) ==> *x == before_expiry(*result),
                      triggers = [(tick(i),)]))]
fn reborrow(x: &mut i32) -> &mut i32 {
    x
}

fn write_through_pledge(x: &mut i32) {
    let r = reborrow(x);
    *r = 5;
    prusti_assert!(tick(0));
    prusti_assert!(*x == 5);
}

#[pure]
#[trusted]
#[ensures(result == (i < n))]
fn below(i: usize, n: usize) -> bool {
    i < n
}

/// A `move` closure, passed to a quantifier directly, captures by value.
#[requires(quantify((), &(move |i: usize| (!below(i, n) || i < n))))]
fn by_value(n: usize) {}

fn by_value_client() {
    by_value(3);
}

// A closure built apart from its quantifier captures the values where it is
// built: `*r` in `c` reads `*a`, as `r` refers to `b` only afterwards.
#[requires({
    let mut r = a;
    let c = |i: usize| (!below(i, *r) || below(i, *a));
    r = b;
    quantify((), &c) && *r == *b
})]
fn built_before_assignment(a: &usize, b: &usize) {}

fn built_before_assignment_client() {
    built_before_assignment(&1, &2);
}

// A closure can be quantified over twice, the second time in the branch of the
// `&&`.
#[requires({
    let c = |i: usize| (!below(i, n) || i < n);
    quantify((), &c) && quantify((), &c)
})]
fn quantified_twice(n: usize) {}

fn quantified_twice_client() {
    quantified_twice(3);
}

// A closure reads its captures in the modes where it is built, not in those
// of its quantifier: built in `old`, it reads `*x` before the call ...
#[requires(*x == 0)]
#[ensures({
    let c = old(|i: usize| (i != 0 || *x == 0));
    quantify((), &c)
})]
fn built_in_old(x: &mut usize) {
    *x = 5;
}

// ... and built outside `old`, after the call.
#[requires(*x == 0)]
#[ensures({
    let c = |i: usize| (i != 0 || *x == 5);
    old(quantify((), &c))
})]
fn quantified_in_old(x: &mut usize) {
    *x = 5;
}
