fn assert_bool(x: bool) {
    assert!(x);  //~ERROR: might not hold
}

fn fail() {
    assert_eq!(1, 8); //~ ERROR: might not hold
}

fn fail2() {
    let x = 5;
    let y = 4 - 1;
    assert_eq!(x, y); //~ ERROR: might not hold
}

macro_rules! my_assert {
    ( $( $args:expr ),* ) => {
        assert!( $( $args ),* )
    };
}

fn assert_from_macro(x: bool) {
    my_assert!(x);  //~ERROR: might not hold
}

fn test(x: i32) {
    let is_pos = x.is_positive();
    assert!(is_pos); //~ERROR: might not hold
}

fn foo() {
    assert!(false);  //~ERROR: might not hold
}

fn bar() {
    assert!(false);  //~ERROR: might not hold
}

fn main() {
    assert!(false);  //~ERROR: might not hold
}
