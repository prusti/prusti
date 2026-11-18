use prusti_contracts::*;

// Regression test for prusti/prusti#1195.
const ARR_SIZE: usize = 50;

struct Array<const SIZE: usize>([i32; SIZE]);

impl<const SIZE: usize> Array<SIZE> {
    #[pure]
    fn size(&self) -> usize {
        SIZE
    }
}

#[ensures(x.size() == 51)] //~ ERROR: postcondition might not hold
fn foo(x: Array<ARR_SIZE>) {}
