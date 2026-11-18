// Regression test for prusti/prusti#1541: panic_any is only allowed in unreachable code.
use prusti_contracts::*;

#[requires(!x)]
fn guarded_panic_any(x: bool) {
    if x {
        std::panic::panic_any(());
    }
}
