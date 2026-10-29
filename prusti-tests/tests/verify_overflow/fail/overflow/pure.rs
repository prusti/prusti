use prusti_contracts::*;

#[pure]
fn sum_pure(a: u32, b: u32) -> u32 {
    a + b //~ ERROR: assertion might fail with "attempt to add with overflow"
}

#[trusted]
#[pure]
fn u32_foo() -> u32 {
    5
}

fn u32_foo_call_1() {
    let n = u32_foo();
    assert!(0 <= n);
    assert!(n <= 4294967295);
    assert!(n == 5); //~ ERROR: might not hold
}

fn u32_foo_call_2() {
    assert!(0 <= u32_foo());
    assert!(u32_foo() <= 4294967295);
    assert!(u32_foo() == 5); //~ ERROR: might not hold
}

#[ensures(0 <= u32_foo() && u32_foo() <= 4294967295)]
#[ensures(u32_foo() == 5)] //~ ERROR: postcondition might not hold
fn u32_foo_call_3() {
}

#[pure]
fn test_minus_i32(x: i32) -> i32 {
    -x //~ERROR: negate with overflow
}

#[pure]
fn test_minus_i64(x: i64) -> i64 {
    -x //~ERROR: negate with overflow
}

#[pure]
fn test_minus_isize(x: isize) -> isize {
    -x //~ERROR: negate with overflow
}

#[pure]
fn test_usize(x: usize) -> usize {
    x - 1 //~ERROR: overflow
}

pub fn test() {
    assert!(test_usize(123) >= 0);
}
