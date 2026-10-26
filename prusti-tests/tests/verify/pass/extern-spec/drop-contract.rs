use prusti_contracts::*;

pub struct Token(u32);

impl Drop for Token {
    fn drop(&mut self) {}
}

// Dropping an `Option<Token>` is only allowed when it is `None`.
#[extern_spec]
impl<T> Drop for Option<T> {
    #[requires(matches!(self, None))]
    fn drop(&mut self);
}

fn scope_end() {
    let _x: Option<Token> = None;
}

fn explicit_drop() {
    let x: Option<Token> = None;
    drop(x);
}

#[requires(matches!(x, None))]
fn dropped_param(x: Option<Token>) {}

#[requires(matches!(x, None))]
fn generic<T>(x: Option<T>) {}

fn moved_out(x: Option<Token>) -> Option<Token> {
    x
}

fn unwrapped(x: Option<Token>) -> Token {
    match x {
        Some(t) => t,
        None => Token(0),
    }
}
