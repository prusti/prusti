// Regression test for prusti/prusti#1512: every value of a fieldless enum is one of its variants.
use prusti_contracts::*;

#[derive(PartialEq, Eq, Copy, Clone)]
#[repr(u16)]
enum T {
    I1 = 1,
    I2 = 2,
    I3 = 3,
}

impl T {
    #[pure]
    #[ensures(matches!(self, T::I1) <==> result == 1)]
    #[ensures(matches!(self, T::I2) <==> result == 2)]
    #[ensures(matches!(self, T::I3) <==> result == 3)]
    fn f(&self) -> u16 {
        match self {
            T::I1 => 1,
            T::I2 => 2,
            T::I3 => 3,
        }
    }
}

#[ensures(forall(|t: T| a == t <==> a.f() != t.f()))] //~ ERROR: postcondition might not hold
fn f_not_injective(a: T) {}

#[ensures(forall(|t: T| t == T::I1 || t == T::I2))] //~ ERROR: postcondition might not hold
fn incomplete() {}
