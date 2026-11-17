use prusti_contracts::*;

fn all_equal(a: [i32; 2]) {
    assert!(a[0] == a[1]);  //~ERROR: might not hold
}

#[pure]
fn literal() -> [u32; 3] {
    [10, 20, 30]
}

#[pure]
fn repeated() -> [u32; 4] {
    [7; 4]
}

#[ensures(literal()[1] == 10)] //~ERROR: postcondition might not hold
fn wrong_literal_element() {}

#[ensures(repeated()[2] == 8)] //~ERROR: postcondition might not hold
fn wrong_repeated_element() {}

#[requires(a.len() > 5)]
fn wrong_subslice_len(a: &[i32]) {
    let s = &a[1..4];
    assert!(s.len() == 4); //~ERROR: might not hold
}

#[requires(a.len() > 5)]
fn wrong_inclusive_subslice_len(a: &[i32]) {
    let s = &a[1..=4];
    assert!(s.len() == 3); //~ERROR: might not hold
}

// `r` may be exhausted, in which case the subslice is empty.
fn exhausted_inclusive_subslice_len(a: &[i32], r: core::ops::RangeInclusive<usize>) {
    if *r.end() < a.len() && *r.start() <= *r.end() {
        let s = &a[r];
        assert!(s.len() > 0); //~ERROR: might not hold
    }
}
