// Regression test for prusti/prusti#1532: array indexing and mutual recursion in pure functions.
use prusti_contracts::*;

#[pure]
fn negate_i32(x: i32) -> i32 {
    [0 - x, x][0]
}

#[ensures(negate_i32(x) == x)] //~ ERROR: postcondition might not hold
fn negate_wrong(x: i32) {}

#[pure]
fn fib(n: isize) -> isize {
    if n <= 1 { 1 } else { fib_race(n) }
}

#[pure]
fn fib_race(n: isize) -> isize {
    fib(n - 1) + fib(n - 2)
}

#[requires(n <= 1)]
#[ensures(fib(n) == 2)] //~ ERROR: postcondition might not hold
fn fib_wrong(n: isize) {}
