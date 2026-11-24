use prusti_contracts::*;

pub struct T(pub u32);

// A parenthesized pattern matches what its inner pattern matches; it is not a
// one-element tuple.
pub fn parenthesized() {
    let _ = closure!(#[requires(x > 0)] |(x): u32| -> u32 { x });
    let _ = closure!(#[requires(f > 0)] |(T(f)): T| -> u32 { f });
}
