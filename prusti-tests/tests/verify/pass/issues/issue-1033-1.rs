use prusti_contracts::*;

pub struct Foo<T>(T);

impl<T: Copy + PartialEq + Eq> Foo<T> {
    #[pure]
    #[ensures(result == (left == right))]
    fn eq(left: T, right: T) -> bool {
        left == right
    }
}
