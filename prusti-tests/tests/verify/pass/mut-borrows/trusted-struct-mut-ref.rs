use prusti_contracts::*;

// A trusted struct is opaque: its predicate abstracts what its mutable
// reference points to, so it has no separate deep snapshot, also as a field of
// another struct.
#[trusted]
pub struct W<'a> {
    r: &'a mut i32,
}

impl<'a> W<'a> {
    #[trusted]
    #[pure]
    fn get(&self) -> i32 {
        *self.r
    }

    #[trusted]
    #[ensures(self.get() == v)]
    fn set(&mut self, v: i32) {
        *self.r = v;
    }
}

#[trusted]
fn mk(r: &mut i32) -> W<'_> {
    W { r }
}

pub struct Outer<'a> {
    w: W<'a>,
    n: u32,
}

fn client(x: &mut i32) {
    let mut w = mk(x);
    w.set(3);
    prusti_assert!(w.get() == 3);
    let mut o = Outer { w, n: 0 };
    o.w.set(4);
    prusti_assert!(o.w.get() == 4);
    prusti_assert!(o.n == 0);
}
