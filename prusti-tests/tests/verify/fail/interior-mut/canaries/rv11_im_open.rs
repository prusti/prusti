//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

#[im_open(x)]
pub fn f(x: &mut Cell<i32>) { //~ ERROR
    prusti_assert!(x.get() == x.get());
}
