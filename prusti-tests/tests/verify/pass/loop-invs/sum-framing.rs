use prusti_contracts::*;

// The step that preserves the invariant of `sum`. Z3 does not reliably prove it among the
// other nonlinear facts of `sum`, so it is proved here on its own.
#[ensures((i + 1) * (i + 1 + 1) / 2 == i * (i + 1) / 2 + (i + 1))]
fn sum_step(i: i32) {}

#[requires(0 <= n)]
#[ensures(result == old(n) * (old(n) + 1) / 2)]
fn sum(n: i32) -> i32 {
    let mut res = 0;
    let mut i = 0;
    while i < n {
        // Note: here we use `n`, not `old(n)`
        body_invariant!(i < n);
        body_invariant!(res == i * (i + 1) / 2);
        // Preserves the invariant: the new `res` is the old one plus the new `i`.
        sum_step(i);
        i = i + 1;
        res = res + i;
    }
    // After the loop, `i == n` follows by linear arithmetic. Without this assertion, Z3
    // proves the postcondition only for some random seeds.
    prusti_assert!(i == n);
    res
}

fn test() {
    assert!(sum(100) == 5050);
    assert!(sum(100) != 5);
    assert!(sum(0) != 1);
    assert!(sum(0) == 0);
    assert!(sum(1) != 0);
    assert!(sum(1) == 1);
}
