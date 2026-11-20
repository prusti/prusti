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

// `'a` is early-bound in `none`, since it only appears in the return type, but
// late-bound in the postcondition item, which takes `result` as an argument. So
// only `none` has a generic argument for it.
#[trusted]
#[ensures(result === None)]
pub fn none<'a, T>() -> Option<&'a T> {
    None
}

fn client_none() {
    let r: Option<&u8> = none();
    prusti_assert!(r === None);
}
