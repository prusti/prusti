//@ compile-flags: -Passert_timeout=45000
// Review round 7: interior-mutable values hidden behind an associated type
// or an `UnsafeCell` type parameter.
use prusti_contracts::*;
use prusti_contracts::cell::Cell;
use core::cell::UnsafeCell;

pub trait Tr { type A; fn bump(a: &Self::A); }
impl Tr for u8 {
    type A = Cell<i32>;
    fn bump(a: &Cell<i32>) { a.set(5) }
}

fn g<T: Tr>(a: &T::A) { T::bump(a) }

pub fn projection(c: &Cell<i32>) {
    c.set(0);
    g::<u8>(c); //~ ERROR
}

pub trait Bump { fn bump(&self); }
impl Bump for Cell<i32> { fn bump(&self) { self.set(5) } }

fn f<T: Bump>(_a: Option<&UnsafeCell<T>>, b: &T) { b.bump() }

pub fn unsafecell_exempt(c: &Cell<i32>) { //~ ERROR
    c.set(0);
    f(None, c);
}

pub fn ctrl(c: &Cell<i32>) {
    c.set(0);
    assert!(false); //~ ERROR
}
