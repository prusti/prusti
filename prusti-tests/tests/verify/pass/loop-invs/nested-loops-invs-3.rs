use prusti_contracts::*;

// The step that preserves the invariants of the two outer loops of `test`. Z3 does not
// reliably prove it among the other nonlinear facts of `test`, so it is proved here on its own.
#[ensures((b + 1) * m == b * m + m)]
fn distribute(b: i32, m: i32) {}

#[requires(n >= 0)]
#[ensures(result == old(n) * (1 + old(n) * (1 + old(n))))]
fn test(n: i32) -> i32 {
    let mut res = 0;
    let mut ia = 0;

    while ia < n {
        body_invariant!(n == old(n));
        body_invariant!(res == ia * (1 + n * (1 + n)));
        body_invariant!(ia < n);
        res += 1;
        let mut ib = 0;

        while ib < n {
            body_invariant!(n == old(n));
            body_invariant!(res == ia * (1 + n * (1 + n)) + 1 + ib * (1 + n));
            body_invariant!(ib < n);
            res += 1;
            let mut ic = 0;

            while ic < n {
                body_invariant!(n == old(n));
                body_invariant!(ic < n && res == ia * (1 + n * (1 + n)) + 1 + ib * (1 + n) + 1 + ic);
                res += 1;
                ic += 1;
            }

            // After the loop, `ic == n` follows by linear arithmetic. Without this and the
            // same assertions for `ib` and `ia` below, Z3 proves the invariants and the
            // postcondition only for some random seeds.
            prusti_assert!(ic == n);
            // Preserves the invariant of this loop: `(ib + 1) * (1 + n)` is the old term plus
            // the `1 + n` increments of this iteration.
            distribute(ib, 1 + n);
            ib += 1;
        }

        // As for `ic` above.
        prusti_assert!(ib == n);
        // Preserves the invariant of this loop, as for `ib` above.
        distribute(ia, 1 + n * (1 + n));
        ia += 1;
    }

    // As for `ic` above.
    prusti_assert!(ia == n);
    res
}
