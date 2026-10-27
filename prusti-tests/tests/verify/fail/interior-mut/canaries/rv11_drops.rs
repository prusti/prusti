//@ compile-flags: -Passert_timeout=45000
// Review round 7: drop contracts of values owned by a box, of overwritten
// fields and referents, and of a generic type's drop specification.
use prusti_contracts::*;
use prusti_contracts::cell::{Cell, RefCell};
use prusti_contracts::rc::Rc;

pub fn box_scope_drop() {
    let a = Rc::new(1);
    {
        let _b = Box::new(Rc::clone(&a));
    }
    assert!(Rc::strong_count(&a) == 2); //~ ERROR
}

pub fn box_mem_drop() {
    let a = Rc::new(1);
    let b = Box::new(Rc::clone(&a));
    drop(b);
    assert!(Rc::strong_count(&a) == 2); //~ ERROR
}

pub fn option_box() {
    let a = Rc::new(1);
    {
        let _b = Some(Box::new(Rc::clone(&a)));
    }
    assert!(Rc::strong_count(&a) == 2); //~ ERROR
}

pub fn box_control() {
    let a = Rc::new(1);
    {
        let _b = Box::new(Rc::clone(&a));
    }
    assert!(false); //~ ERROR
}

pub struct S { pub r: Rc<i32>, pub n: u8 }

pub fn overwrite_field() {
    let a = Rc::new(1);
    let mut s = S { r: Rc::clone(&a), n: 0 };
    s.r = Rc::new(2);
    assert!(Rc::strong_count(&a) == 2); //~ ERROR
}

pub fn overwrite_tuple_field() {
    let a = Rc::new(1);
    let mut t = (Rc::clone(&a), 0u8);
    t.0 = Rc::new(2);
    assert!(Rc::strong_count(&a) == 2); //~ ERROR
}

pub fn via_mutref() {
    let a = Rc::new(1);
    let mut b = Rc::clone(&a);
    {
        let r = &mut b;
        *r = Rc::new(2);
    }
    assert!(Rc::strong_count(&a) == 2); //~ ERROR
}

pub fn overwrite_guard() {
    let c = RefCell::new(1);
    let d = RefCell::new(2);
    let mut t = (c.borrow(), 0u8);
    t.0 = d.borrow();
    let _m = c.borrow_mut();
}

pub struct W<T>(pub T, pub u32);

#[extern_spec]
impl<T> Drop for W<T> {
    #[requires(self.1 == 0)]
    fn drop(&mut self);
}

pub fn drop_w() {
    let c = Cell::new(1);
    let _w = W(String::new(), 0);
    c.set(2);
}

pub fn drop_w_bad() {
    let c = Cell::new(1);
    let _w = W(String::new(), 1);
    c.set(2);
} //~ ERROR

#[ensures(w.1 == old(w.1) + 1)]
fn bump<T>(w: &mut W<T>) { w.1 += 1; }

pub fn generic_mutref() {
    let c = Cell::new(1);
    let mut w = W(String::new(), 0);
    bump(&mut w);
    assert!(w.1 == 1);
    w.1 = 0;
    c.set(2);
}
