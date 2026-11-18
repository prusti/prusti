// Regression test for prusti/prusti#1504: string constants passed to println.
use prusti_contracts::*;

#[ensures(result == n + 1)] //~ ERROR: postcondition might not hold
pub fn print_and_return_wrong(n: u32) -> u32 {
    println!("value: {}", n);
    n
}
