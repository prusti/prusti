//@ known-failure
// Regression test for prusti/prusti#1544: the same program as the pass test, but with a pledge
// that changes the length, so the loop invariant on the length fails.
use prusti_contracts::*;

pub struct VecWrapper<T> {
    v: Vec<T>,
}

impl<T: Clone> VecWrapper<T> {
    #[trusted]
    #[ensures(result.len() == size)]
    pub fn new(value: T, size: usize) -> Self {
        VecWrapper { v: vec![value; size] }
    }

    #[trusted]
    #[pure]
    pub fn len(&self) -> usize {
        self.v.len()
    }

    #[trusted]
    #[requires(0 <= index && index < self.len())]
    pub fn index(&self, index: usize) -> &T {
        &self.v[index]
    }

    #[trusted]
    #[requires(0 <= index && index < self.len())]
    #[after_expiry(self.len() == old(self.len()) + 1)]
    pub fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.v[index]
    }
}

fn doors() {
    let mut door_open = VecWrapper::new(false, 100);
    let mut pass = 1;
    while pass < 100 {
        body_invariant!(pass < 100);
        body_invariant!(1 <= pass);
        body_invariant!(door_open.len() == 100); //~ERROR: loop invariant might not hold after a loop iteration
        let mut door = pass;
        while door <= 100 {
            body_invariant!(door <= 100);
            body_invariant!(1 <= door);
            body_invariant!(door_open.len() == 100);
            let door_state = !door_open.index(door - 1);
            let new_door_state = door_open.index_mut(door - 1);
            let moved_state = new_door_state;
            *moved_state = door_state;
            door += pass;
        }
        pass += 1;
    }
}
