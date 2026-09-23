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
    let _x = Some(Token(1));
} //~ ERROR: the precondition of the drop contract of `std::option::Option<Token>` might not hold

fn explicit_drop() {
    let x = Some(Token(1));
    drop(x); //~ ERROR: the precondition of the drop contract of `std::option::Option<Token>` might not hold
}

fn dropped_param(x: Option<Token>) {} //~ ERROR: the precondition of the drop contract of `std::option::Option<Token>` might not hold
