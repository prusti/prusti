fn test(x: i32) {
    let is_pos = x.is_positive();
    assert!(is_pos); //~ERROR: might not hold
}
