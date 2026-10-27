//@ compile-flags: -Passert_timeout=45000
// Review round 13: places a loop only reads keep a part of their permission
// outside the loop, so their values (and the atoms keyed on them) survive it.
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub fn read_ref(x: &i32) {
    let a = *x;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _b = *x;
        i += 1;
    }
    assert!(*x == a);
}

pub fn read_ref_false(x: &i32) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        let _b = *x;
        i += 1;
    }
    assert!(false); //~ ERROR
}

pub fn cell_param(c: &Cell<i32>) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        c.set(i);
        i += 1;
    }
    let _x = c.get();
}

pub fn cell_contents_change(c: &Cell<i32>) {
    c.set(1);
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        c.set(2);
        i += 1;
    }
    assert!(c.get() == 1); //~ ERROR
}

pub fn cell_local_ref() {
    let c = Cell::new(0);
    let d = Cell::new(5);
    let r = &c;
    let mut i = 0;
    while i < 10 {
        body_invariant!(i >= 0);
        r.set(i);
        i += 1;
    }
    let _x = c.get();
    assert!(d.get() == 5);
}

pub struct S { pub a: Cell<i32> }

impl S {
    pub fn run(&self) {
        let mut i = 0;
        while i < 10 {
            body_invariant!(i >= 0);
            self.a.set(i);
            i += 1;
        }
        let _x = self.a.get();
    }
}

// Review round 14: an array a loop only reads stays at full permission (its
// repacks need it).
pub fn read_array(arr: [u32; 3]) {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i <= 10);
        let _y = arr[1];
        i += 1;
    }
}
