//@ compile-flags: -Popt_in_verification=false
use prusti_contracts::*;

fn i_am_not_verified() {
    unreachable!(); //~ ERROR: might not hold
}

#[verified]
fn i_am_verified() {
    assert!(1 == 2); //~ ERROR: might not hold
}

#[verified]
#[trusted]
fn i_am_trusted() {
    unreachable!();
}
