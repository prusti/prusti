// Regression test for prusti/prusti#738: a pure function with casts between `isize` and `usize`,
// called in specs.
use prusti_contracts::*;

#[derive(Clone, Copy)]
pub struct A {
    count: isize,
}

impl A {
    #[pure]
    #[ensures(result <= isize::MAX as usize)]
    pub const fn len(&self) -> usize {
        if self.count < 0 {
            ((1_isize + self.count) + isize::MAX) as usize
        } else {
            self.count as usize
        }
    }
}

#[requires(slice.len() > 0)]
#[requires(slice[0].count == 3)]
#[ensures(slice[0].len() == 3)]
fn len_of_first(slice: &[A]) {}

#[requires(slice.len() > 0)]
#[requires(slice[0].count == 3)]
#[ensures(slice[0].len() == 4)] //~ ERROR: postcondition might not hold
fn len_of_first_wrong(slice: &[A]) {}
