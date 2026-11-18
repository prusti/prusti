use prusti_contracts::*;

// Regression test for prusti/prusti#445.
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

fn wrong_claim(a: &mut A) {
    a.x = 5;
    {
        let r = a.borrow();
        *r = 7;
    }
    prusti_assert!(a.x == 8); //~ERROR: the asserted expression might not hold
}
