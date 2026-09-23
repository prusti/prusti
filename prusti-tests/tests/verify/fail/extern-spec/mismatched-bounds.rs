use prusti_contracts::*;

// The spec would apply to `Option<T>::is_some` for every `T`, not only
// `T: Copy`.
#[extern_spec]
impl<T: Copy> core::option::Option<T> {
    #[pure]
    #[trusted]
    fn is_some(&self) -> bool; //~ ERROR: Invalid external specification
}
