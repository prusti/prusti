fn foo(x: i32) -> i32 {
    unreachable!(); //~ERROR: might not hold
}
