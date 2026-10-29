macro_rules! my_assert {
    ( $( $args:expr ),* ) => {
        assert!( $( $args ),* )
    };
}

fn foo(x: bool) {
    my_assert!(x);  //~ERROR: might not hold
}
