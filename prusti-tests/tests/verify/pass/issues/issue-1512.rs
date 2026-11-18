use prusti_contracts::*;

// Regression test for prusti/prusti#1512: every value of a fieldless enum is one of its variants.
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

#[ensures(a.f() == b.f() <==> a == b)]
fn f_injective(a: T, b: T) {}

#[ensures(forall(|t: T| a == t <==> a.f() == t.f()))]
fn f_injective_forall(a: T) {}

#[ensures(forall(|t: T| t == T::I1 || t == T::I2 || t == T::I3))]
fn complete() {}
