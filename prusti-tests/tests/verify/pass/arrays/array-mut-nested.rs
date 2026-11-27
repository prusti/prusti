//@ known-failure
use prusti_contracts::*;

// Borrows of an element of a nested array at runtime indices. The index
// temporaries die before the borrow expires, when the elements are folded
// back, and the PCG labels a dead index local only in the guide of the
// expansion it indexes, not in the places that contain it. Folding `a[i][j]`
// back into `a[i]` would read the lost value of `i`, so this is reported as
// unsupported until the PCG labels dead index locals in places too.

#[requires(i < 4 && j < 3 && k < 4 && k != i)]
fn nested(a: &mut [[i32; 3]; 4], i: usize, j: usize, k: usize) {
    let old = a[k][j];
    let r = &mut a[i][j];
    *r = 5;
    assert!(a[i][j] == 5);
    assert!(a[k][j] == old);
}

#[requires(i < 4 && j < 3)]
fn nested_local(i: usize, j: usize) {
    let mut a = [[0i32; 3]; 4];
    let r = &mut a[i][j];
    *r = 5;
    assert!(a[i][j] == 5);
}

#[requires(i < 4 && j < 3)]
#[after_expiry(a[i][j] == before_expiry(*result))]
fn get_nested(a: &mut [[i32; 3]; 4], i: usize, j: usize) -> &mut i32 {
    &mut a[i][j]
}

#[requires(i < 4 && j < 3)]
fn get_nested_client(a: &mut [[i32; 3]; 4], i: usize, j: usize) {
    let r = get_nested(a, i, j);
    *r = 7;
    assert!(a[i][j] == 7);
}
