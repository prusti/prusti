// Regression test for prusti/prusti#786: loop condition calling a pure method.
use prusti_contracts::*;

pub struct VecWrapper {
    v: Vec<usize>,
}

impl VecWrapper {
    #[trusted]
    #[pure]
    pub fn len(&self) -> usize {
        self.v.len()
    }
}

#[requires(p.len() > 0)]
fn loop_condition_with_len_probe(p: &VecWrapper) {
    let mut x = 0;
    while x <= p.len() - 1 {
        assert!(false); //~ ERROR: precondition of `core::panicking::panic`
    }
}
