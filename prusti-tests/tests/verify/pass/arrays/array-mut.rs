use prusti_contracts::*;

fn main() {
    let mut a = [1, 2, 3];
    a[1] = 23;

    assert!(a[0] == 1);
    assert!(a[1] == 23);
    assert!(a[2] == 3);
}

fn compound_assignment() {
    let mut a = [0; 3];
    a[0] += 1;
}

fn assignment_after_call() {
    println!("Hi");
    let mut t = [0];
    t[0] = 1;
}

fn looping() {
    let mut a = [0; 3];

    let mut i = 0;
    while i < 3 {
        body_invariant!(0 <= i && i < 3);
        a[i] = i;

        i += 1;
    }

    assert!(i == 3);
}

fn array_mut(mut a: [i32; 3]) {
    let a0 = a[0];
    let b = &mut a[0];
    assert!(*b == a0);
    *b = 42;
    // b expires here
    assert!(a[0] == 42);
}

// Writes at a variable index inside branches: the array is unfolded at the
// index and folded back after the index temporary has died.
fn set_if(a: &mut [i32; 4], i: usize) {
    if i < 4 {
        a[i] = 1;
        prusti_assert!(a[i] == 1);
    }
}

fn set_both(a: &mut [i32; 4], i: usize, c: bool) {
    if i < 4 {
        if c {
            a[i] = 1;
        } else {
            a[i] = 2;
        }
        prusti_assert!(a[i] > 0);
    }
}

// Returning a borrow of an element: the package of the result's wand folds
// the element back at the index.
fn get_or_first(a: &mut [i32; 4], i: usize) -> &mut i32 {
    if i < 4 { &mut a[i] } else { &mut a[0] }
}

#[requires(i < 4)]
#[ensures(*result == old(a[i]))]
#[after_expiry(a[i] == before_expiry(*result))]
fn get(a: &mut [i32; 4], i: usize) -> &mut i32 {
    &mut a[i]
}

#[requires(i < 4 && j < 4 && i != j)]
#[after_expiry(a[i] == before_expiry(*result) && a[j] == old(a[j]))]
fn get_framed(a: &mut [i32; 4], i: usize, j: usize) -> &mut i32 {
    &mut a[i]
}

#[requires(i < 4 && j < 4 && i != j)]
fn get_framed_client(a: &mut [i32; 4], i: usize, j: usize) {
    let old_j = a[j];
    let r = get_framed(a, i, j);
    *r = 7;
    assert!(a[i] == 7);
    assert!(a[j] == old_j);
}

struct S {
    arr: [u8; 8],
    x: u32,
}

#[requires(i < 8)]
#[after_expiry(s.arr[i] == before_expiry(*result) && s.x == old(s.x))]
fn get_field(s: &mut S, i: usize) -> &mut u8 {
    &mut s.arr[i]
}
