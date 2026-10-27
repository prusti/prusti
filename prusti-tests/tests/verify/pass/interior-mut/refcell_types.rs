//@ compile-flags: -Passert_timeout=45000
// `RefCell` with different content types, structs mixing `Cell` and
// `RefCell` fields, and sharing. The cases Prusti does not verify yet are in
// `refcell_types_failing.rs`.

use prusti_contracts::cell::{Cell, RefCell};

struct Pair {
    a: i32,
    b: i32,
}

fn non_copy_content() {
    let c = RefCell::new(Pair { a: 1, b: 2 });
    {
        let mut m = c.borrow_mut();
        m.a = 3;
    }
    let r = c.borrow();
    assert!(r.a == 3);
    assert!(r.b == 2);
}

struct Mixed {
    counter: Cell<u32>,
    data: RefCell<i32>,
}

fn mixed_struct_through_shared_ref() {
    let m = Mixed {
        counter: Cell::new(0),
        data: RefCell::new(10),
    };
    let r = &m;
    r.counter.set(r.counter.get() + 1);
    *r.data.borrow_mut() += 1;
    assert!(r.counter.get() == 1);
    assert!(*r.data.borrow() == 11);
}

fn two_aliases_of_local() {
    let c = RefCell::new(0);
    let r1 = &c;
    let r2 = &c;
    r1.replace(1);
    assert!(*r2.borrow() == 1);
    r2.replace(2);
    assert!(*r1.borrow() == 2);
}

fn main() {
    non_copy_content();
    mixed_struct_through_shared_ref();
    two_aliases_of_local();
}
