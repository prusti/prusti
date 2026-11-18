// Regression test for prusti/prusti#943: rustc's coherence checks run before verification.
use prusti_contracts::*;

impl std::fmt::Display for Vec<i32> { //~ ERROR: only traits defined in the current crate can be implemented for types defined outside of the crate
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Ok(())
    }
}

#[ensures(false)]
pub fn unsound() {}
