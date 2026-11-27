//@ known-failure
use prusti_contracts::*;

// Borrows of an element of a nested array at runtime indices. The index
// temporaries die before the borrow expires, when the elements are folded
// back, and the PCG labels a dead index local only in the guide of the
// expansion it indexes, not in the places that contain it. Folding `a[i][j]`
// back into `a[i]` would read the lost value of `i`, so this is reported as
// unsupported until the PCG labels dead index locals in places too.

#[requires(i < 4 && j < 3)]
fn nested_borrow(a: &mut [[i32; 3]; 4], i: usize, j: usize) {
    let r = &mut a[i][j];
    *r = 5;
    assert!(a[i][j] == 6); //~ERROR: might not hold
}

#[requires(i < 4 && j < 3)]
#[after_expiry(a[i][j] == before_expiry(*result) + 1)] //~ERROR: pledge in the postcondition might not hold
fn get_nested(a: &mut [[i32; 3]; 4], i: usize, j: usize) -> &mut i32 {
    &mut a[i][j]
}
