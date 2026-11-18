fn add_u32(x: u32, y: u32) -> u32 {
    x + y //~ ERROR: assertion might fail with "attempt to add with overflow"
}

fn div_isize(x: isize, y: isize) -> isize {
    x / y //~ ERROR: assertion might fail with "attempt to divide by zero"
}

fn div_u64(x: u64, y: u64) -> u64 {
    x / y //~ ERROR: assertion might fail with "attempt to divide by zero"
}

// Regression test for prusti/prusti#881 (with overflow checks off: verify/pass/issues/issue-881.rs).
fn negate_i32(x: i32) -> i32 {
    -x //~ ERROR: assertion might fail with "attempt to negate with overflow"
}
