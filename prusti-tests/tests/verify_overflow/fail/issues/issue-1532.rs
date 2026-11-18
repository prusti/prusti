// Regression test for prusti/prusti#1532: overflow in these pure functions is reported as an
// ordinary verification error.
use prusti_contracts::*;

#[pure]
fn negate_i32(x: i32) -> i32 {
    [0 - x, x][0] //~ ERROR: attempt to subtract with overflow
}

#[pure]
fn fib(n: isize) -> isize {
    if n <= 1 { 1 } else { fib_race(n) }
}

#[pure]
fn fib_race(n: isize) -> isize {
    fib(n - 1) + fib(n - 2) //~ ERROR: attempt to subtract with overflow
}

#[pure]
fn add_sub(x: i64) -> i64 {
    x + i64::MAX - x //~ ERROR: attempt to add with overflow
}
