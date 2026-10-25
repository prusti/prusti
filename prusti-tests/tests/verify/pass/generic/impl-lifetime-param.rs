use prusti_contracts::*;

// The specs of a method in an impl with a lifetime parameter: `T` follows
// `'a` in the generics of both the method and its spec items.
pub struct Wrapper<'a, T> {
    value: &'a T,
    n: usize,
}

impl<'a, T> Wrapper<'a, T> {
    #[pure]
    pub fn n(&self) -> usize {
        self.n
    }

    #[ensures(result == self.n())]
    pub fn get_n(&self) -> usize {
        self.n
    }
}

fn client(w: &Wrapper<i32>) {
    let n = w.get_n();
    assert!(n == w.n());
}
