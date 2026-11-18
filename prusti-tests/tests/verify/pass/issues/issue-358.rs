// Regression test for prusti/prusti#358: a closure capturing a struct field, passed to a
// std iterator adapter, must not crash the encoder.
use prusti_contracts::*;

struct Params {
    letter: char,
    password: String,
}

pub fn count_letter(params: Params) -> usize {
    params.password.chars().filter(|c| c == &params.letter).count()
}
