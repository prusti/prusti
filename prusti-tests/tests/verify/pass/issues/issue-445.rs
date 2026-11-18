// Regression test for prusti/prusti#445: a `&mut self` method returning a `&mut` reference.
use prusti_contracts::*;

pub struct A {
    pub x: i32,
}

impl A {
    #[ensures(*result == old(self.x))]
    #[after_expiry(self.x == before_expiry(*result))]
    pub fn borrow(&mut self) -> &mut i32 {
        &mut self.x
    }
}

fn caller(a: &mut A) {
    a.x = 5;
    {
        let r = a.borrow();
        prusti_assert!(*r == 5);
        *r = 7;
    }
    prusti_assert!(a.x == 7);
}
