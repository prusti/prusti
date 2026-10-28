use prusti_contracts::*;

// A call through the trait assumes the trait's pledges for every impl, so an
// impl may not declare pledges of its own.
trait Pick {
    #[after_expiry(*x == before_expiry(*result))]
    fn pick<'a>(&self, x: &'a mut i32, y: &'a mut i32) -> &'a mut i32;
}

struct S;

#[refine_trait_spec]
impl Pick for S {
    #[after_expiry(*y == before_expiry(*result))] //~ ERROR: pledges on `<S as Pick>::pick`, which refines the specification of a trait method, are not supported
    fn pick<'a>(&self, _x: &'a mut i32, y: &'a mut i32) -> &'a mut i32 {
        y
    }
}

pub fn client() {
    let mut a = 0;
    let mut b = 0;
    let r = S.pick(&mut a, &mut b);
    *r = 7;
    assert!(a == 7);
}
