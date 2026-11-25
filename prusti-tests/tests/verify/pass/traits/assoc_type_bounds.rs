use prusti_contracts::*;

pub trait Model {
    type State: Copy;
}

pub struct Wrapper<T>(T);

pub trait Tr {
    #[pure]
    fn f(&self) -> u32;
}

#[refine_trait_spec]
impl<T: Copy> Tr for Wrapper<T> {
    #[pure]
    #[ensures(result == 5)]
    fn f(&self) -> u32 {
        5
    }
}

pub fn via_assoc<M: Model>(w: &Wrapper<M::State>) {
    assert!(w.f() == 5);
}

fn main() {}
