fn add_u32(x: u32, y: u32) -> u32 {
    x + y //~ ERROR: assertion might fail with "attempt to add with overflow"
}

fn div_isize(x: isize, y: isize) -> isize {
    x / y //~ ERROR: assertion might fail with "attempt to divide by zero"
}

fn div_u64(x: u64, y: u64) -> u64 {
    x / y //~ ERROR: assertion might fail with "attempt to divide by zero"
}
