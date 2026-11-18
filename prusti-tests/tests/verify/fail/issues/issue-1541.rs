// Regression test for prusti/prusti#1541: a reachable panic_any is reported.
fn reachable_panic_any() {
    std::panic::panic_any(()); //~ ERROR: precondition might not hold
}
