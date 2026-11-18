use prusti_contracts::*;

// Regression test for prusti/prusti#651.
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum BasicEnum {
    Variant1(u32),
    Variant2(u32),
}

pub struct VecWrapper {
    v: Vec<BasicEnum>,
}

impl VecWrapper {
    #[trusted]
    #[pure]
    pub fn len(&self) -> usize {
        self.v.len()
    }

    #[trusted]
    #[pure]
    #[requires(index < self.len())]
    pub fn lookup(&self, index: usize) -> BasicEnum {
        self.v[index]
    }

    #[trusted]
    #[ensures(self.len() == old(self.len()) + 1)]
    #[ensures(self.lookup(old(self.len())) == value)]
    pub fn push(&mut self, value: BasicEnum) {
        self.v.push(value);
    }
}

fn push_then_lookup_other(vw: &mut VecWrapper, a: BasicEnum, b: BasicEnum) {
    let n = vw.len();
    vw.push(a);
    prusti_assert!(vw.lookup(n) == b); //~ ERROR: the asserted expression might not hold
}

fn push_probe(vw: &mut VecWrapper, a: BasicEnum) {
    vw.push(a);
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}
