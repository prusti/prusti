use prusti_contracts::*;
use std::cmp::Ordering;

#[pure]
fn max(x: i32, y: i32) -> i32 {
    if x < y { y } else { x }
}

#[pure]
#[requires(0 <= n && n <= 2)]
fn magic(n: i32) -> i32 {
    match n {
        -1 => 123,
        0 => -1,
        1 => 1,
        2 => 42,
        _ => unreachable!()
    }
}

#[ensures(magic(0) == -1)]
#[ensures(magic(1) == 1)]
#[ensures(magic(2) == 42)]
fn main() {}

#[pure]
fn cmp(x: i32, y: i32) -> Ordering {
    Ordering::Less
}

#[derive(Clone, Copy)]
struct MyWrapper(u32);

impl MyWrapper {
    #[pure]
    #[ensures(self === MyWrapper(result))]
    fn unwrap(self) -> u32 {
        self.0
    }
}

fn test(x: MyWrapper) -> u32 {
    x.unwrap()
}

/// A local that one branch leaves unchanged keeps its value from before.
#[pure]
fn keep_or_set(a: u32, c: bool) -> u32 {
    let mut x = a + 1;
    if c {
        x = 5
    }
    x
}

#[requires(a < 100)]
#[ensures(!c ==> keep_or_set(a, c) == a + 1)]
fn keep_or_set_spec(a: u32, c: bool) {}
