//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub struct G<'a> { pub r: &'a mut i32, pub c: Cell<i32> }

pub fn g<'a>(x: &'a mut i32) -> G<'a> {
    G { r: x, c: Cell::new(3) }
}

pub fn client2() {
    let mut v = 1;
    let gg = g(&mut v);
    *gg.r = 7;
    gg.c.set(4);
    drop(gg);
    v = 3;
}

pub fn client3() {
    let mut v = 1;
    let gg = g(&mut v);
    gg.c.set(4);
    let x = gg.c.get();
    assert!(x == 4);
}

pub fn ctl() {
    let mut v = 1;
    let gg = g(&mut v);
    gg.c.set(4);
    assert!(false); //~ ERROR
}
