use prusti_contracts::*;

// Regression test for prusti/prusti#1504.
#[ensures(result == n + 1)] //~ ERROR: postcondition might not hold
pub fn print_and_return_wrong(n: u32) -> u32 {
    println!("value: {}", n);
    n
}
