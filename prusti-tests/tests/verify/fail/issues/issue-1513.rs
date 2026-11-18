use prusti_contracts::*;

// Regression test for prusti/prusti#1513.
#[derive(Copy, Clone)]
#[repr(u32)]
enum T {
    A = 1,
    B = 2,
}

impl T {
    #[ensures(result == 3)] //~ ERROR: postcondition might not hold
    fn whoami(&self) -> u32 {
        *self as u32
    }
}

fn wrong_cast() {
    let y = T::B;
    prusti_assert!(y as u32 == 1); //~ ERROR: might not hold
}
