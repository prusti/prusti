//@ known-failure
// Regression test for prusti/prusti#358: the body is encoded, so a false claim after the
// closure-taking calls is reported.
use prusti_contracts::*;

struct Params {
    letter: char,
    password: String,
}

pub fn count_letter(params: Params) -> usize {
    let r = params.password.chars().filter(|c| c == &params.letter).count();
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
    r
}
