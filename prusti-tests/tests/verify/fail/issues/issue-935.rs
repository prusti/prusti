// Regression test for prusti/prusti#935: calls on a value of a nested generic type.
use prusti_contracts::*;

struct Vector<T>(T);
struct Wrapper<T>(T);

impl<T> Vector<T> {
    #[pure]
    fn len(&self) -> usize {
        1
    }

    #[requires(idx < self.len())]
    fn index(&self, idx: usize) {}
}

fn out_of_range<T>(vec: &Vector<Wrapper<T>>) {
    vec.index(1); //~ ERROR: precondition might not hold
}

fn wrong_len<T>(vec: &Vector<Wrapper<T>>) {
    prusti_assert!(vec.len() == 2); //~ ERROR: the asserted expression might not hold
}
