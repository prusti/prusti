fn foo() {
    assert!(false);  //~ERROR: might not hold
}

fn bar() {
    assert!(false);  //~ERROR: might not hold
}

fn main() {
    assert!(false);  //~ERROR: might not hold
}
