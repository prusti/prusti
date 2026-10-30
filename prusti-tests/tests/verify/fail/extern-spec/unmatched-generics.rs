use prusti_contracts::*;

// `T` stands for no generic parameter of `Option<T>::is_some`.
#[extern_spec]
impl<T> core::option::Option<Vec<T>> {
    #[pure]
    #[trusted]
    fn is_some(&self) -> bool; //~ ERROR: Invalid external specification
}
