// Regression test for prusti/prusti#1513: casting a `#[repr(u32)]` enum to an integer.
use prusti_contracts::*;

#[derive(Copy, Clone)]
#[repr(u32)]
enum T {
    A = 1,
    B = 2,
}

impl T {
    #[ensures(*self as u32 == result)]
    fn whoami(&self) -> u32 {
        *self as u32
    }
}

fn cast_values() {
    let x = T::A;
    prusti_assert!(x as u32 == 1);
    let y = T::B;
    prusti_assert!(y as u32 == 2);
}
