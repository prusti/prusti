use prusti_contracts::*;

fn main() {}


fn all_equal(a: [i32; 2]) {
    assert!(a[0] == a[1]);  //~ERROR: asserted expression might not hold
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
