fn test(x: u64, y: u64) -> u64 {
    x / y //~ ERROR: assertion might fail with "attempt to divide by zero"
}
