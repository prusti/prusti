fn test(x: isize, y: isize) -> isize {
    x / y //~ ERROR: assertion might fail with "attempt to divide by zero"
}
