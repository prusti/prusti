use prusti_contracts::*;

#[pure]
fn f_tuple() -> (u8, u8) {
    (0, 0)
}

#[derive(Copy, Clone)]
struct X(u8, u8);

#[pure]
fn f_struct() -> X {
    X(0, 0)
}

#[pure]
fn f_parenthesized() -> (u8) {
    (0)
}
