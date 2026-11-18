// Regression test for prusti/prusti#1084: binding an associated type in a bound.
use prusti_contracts::*;

trait Fn<A> {
    type Output;

    #[pure]
    fn postcondition(&self, arg: &A, res: &Self::Output) -> bool;

    #[ensures(self.postcondition(&arg, &result))]
    fn call(&self, arg: A) -> Self::Output;
}

#[ensures(f.postcondition(&arg, &result))]
fn call<T: PartialEq, O, F: Fn<T, Output = O>>(arg: T, f: &F) -> O {
    f.call(arg)
}
