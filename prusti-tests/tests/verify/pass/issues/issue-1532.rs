use prusti_contracts::*;

// Regression test for prusti/prusti#1532: no internal errors for these pure functions.
#[pure]
fn negate_i32(x: i32) -> i32 {
    [0 - x, x][0]
}

#[ensures(negate_i32(x) == 0 - x)]
fn use_negate(x: i32) {}

#[pure]
fn fib(n: isize) -> isize {
    if n <= 1 { 1 } else { fib_race(n) }
}

#[pure]
fn fib_race(n: isize) -> isize {
    fib(n - 1) + fib(n - 2)
}

#[requires(n <= 1)]
#[ensures(fib(n) == 1)]
fn use_fib(n: isize) {}

#[pure]
fn add_sub(x: i64) -> i64 {
    x + i64::MAX - x
}

fn calls() {
    negate_i32(i32::MIN);
    fib(5);
    add_sub(1);
}
