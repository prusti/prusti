//@ compile-flags: -Wwarnings
// The verifier's warnings are reported at their position. The harness
// silences warnings (`-Awarnings`), which `-Wwarnings` undoes.
use prusti_contracts::*;

// The verifier finds no term in the quantifier's body to use as a trigger.
// The fail group needs an error besides the warning.
#[requires(forall(|a: i32| exists(|b: i32| b == 5 ==> b < a)))] //~WARN: the verifier reports: No triggers provided or inferred for quantifier
pub fn no_trigger() {
    prusti_assert!(false); //~ERROR: might not hold
}
