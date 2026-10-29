use prusti_contracts::*;

#[pure]
#[ensures(result == a[0])]
fn first(a: &[i32; 5]) -> i32 {
    a[0]
}
