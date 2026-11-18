use prusti_contracts::*;

#[ensures(result == 1)]
const fn foo() -> usize {
    1
}

// The closures are nested in the anonymous constants of the array lengths.
// Those of a specification or of a trusted function are not verified, so
// their `assert!(false)` is not reported.
#[ensures(result == [0u8; { let _f = |x: u8| { assert!(false); x }; 3 }].len())]
fn closure_in_length() -> usize {
    [0u8; { let _g = |x: usize| x; 3 }].len()
}

#[trusted]
fn trusted_closure_in_length() -> usize {
    [0u8; { let _g = |x: u8| { assert!(false); x }; 3 }].len()
}

fn main() {
    let bar: [usize; foo()] = [1];
    assert!(bar[0] == foo());
    assert!(closure_in_length() == 3);
}
