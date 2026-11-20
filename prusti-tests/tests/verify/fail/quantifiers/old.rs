use prusti_contracts::*;

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

/// Satisfiable only if `old` inside the quantifier reads the pre-state; if it
/// read the post-state, the postcondition would be `x != x` and hand the caller
/// `false`.
#[trusted]
#[requires(0 < arr.len())]
#[ensures(arr.len() == old(arr.len()))]
#[ensures(forall(|i: usize| i < arr.len() ==> arr.lookup(i) != old(arr.lookup(i)),
                 triggers = [(arr.lookup(i),)]))]
fn change_all(arr: &mut VecI32) {}

#[requires(0 < arr.len())]
fn no_contradiction(arr: &mut VecI32) {
    change_all(arr);
    let y = arr.lookup(0);
    prusti_assert!(y != y); //~ ERROR: the asserted expression might not hold
}

#[pure]
#[trusted]
#[ensures(result)]
fn tick(_i: usize) -> bool {
    true
}

/// Satisfiable only if `before_expiry` inside the quantifier reads the target
/// of the borrow before it expires.
#[trusted]
#[after_expiry(forall(|i: usize| tick(i) ==> *x != before_expiry(*result),
                      triggers = [(tick(i),)]))]
fn change_on_expiry(x: &mut i32) -> &mut i32 {
    x
}

fn pledge_no_contradiction(x: &mut i32) {
    let r = change_on_expiry(x);
    *r = 5;
    prusti_assert!(tick(0));
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

/// `old` in an assertion reads the state at the function's entry.
#[requires(0 < arr.len())]
fn assertion_reads_entry_state(arr: &mut VecI32) {
    change_all(arr);
    prusti_assert!(old(arr.lookup(0)) == arr.lookup(0)); //~ ERROR: the asserted expression might not hold
}

#[trusted]
#[requires(0 < arr.len())]
#[ensures(arr.len() == old(arr.len()))]
#[ensures(arr.lookup(0) == old(arr.lookup(0)) + 1)]
fn bump_first(arr: &mut VecI32) {}

/// `old` in a quantified loop invariant reads the state at the function's
/// entry, which `bump_first` changed.
#[requires(0 < arr.len())]
fn loop_invariant_reads_entry_state(arr: &mut VecI32) {
    bump_first(arr);
    let mut k = 0;
    while k < 3 {
        body_invariant!(forall(|i: usize| i == 0 ==> arr.lookup(i) == old(arr.lookup(i)), //~ ERROR: loop invariant might not hold in the first loop iteration
                               triggers = [(arr.lookup(i),)]));
        k += 1;
    }
}

/// `old` in an inner quantifier reads the pre-state too: the claim that
/// nothing changed does not follow from `change_all`.
#[requires(0 < arr.len())]
#[ensures(forall(|i: usize| forall(|j: usize| (i < arr.len() && j == i) ==> arr.lookup(i) == old(arr.lookup(j)), //~ ERROR: postcondition might not hold
                                   triggers = [(arr.lookup(j),)]),
                 triggers = [(arr.lookup(i),)]))]
fn nested_frame(arr: &mut VecI32) {
    change_all(arr);
}
