//@ edition: 2018
macro_rules! assert {
    ( $( $args:expr ),* ) => {
        panic!( $( $args ),* )
    };
}

fn foo(x: bool) {
    assert!(x);  //~ERROR: might not hold
}
