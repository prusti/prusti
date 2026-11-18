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

fn loop_condition_with_len(p: &VecWrapper) {
    let mut x = 0;
    while x <= p.len() - 1 {}
}
