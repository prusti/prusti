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

pub trait Out {
    type O;
}

pub trait ProjModel {
    type State: Out<O = u32>;
}

pub struct OutWrapper<T>(T);

#[refine_trait_spec]
impl<T: Out<O = u32>> Tr for OutWrapper<T> {
    #[pure]
    #[ensures(result == 5)]
    fn f(&self) -> u32 {
        5
    }
}

pub fn via_projection_bound<M: ProjModel>(w: &OutWrapper<M::State>) {
    assert!(w.f() == 5);
}

pub trait Family {
    type Of<U: Copy>: Copy;
}

pub fn via_gat<F: Family>(w: &Wrapper<F::Of<u32>>) {
    assert!(w.f() == 5);
}
