use prusti_contracts::*;

// The impl's parameters in a different order than the target's.
#[extern_spec]
impl<E, T> Result<T, E> {
    #[trusted]
    fn ok(self) -> Option<T>; //~ ERROR: Invalid external specification
}

// The spec would apply to `Option<T>::is_some` for every `T`, not only for
// vectors.
#[extern_spec]
impl<T> Option<Vec<T>> {
    #[pure]
    #[trusted]
    fn is_some(&self) -> bool; //~ ERROR: Invalid external specification
}

// The same for a type without parameters.
#[extern_spec]
impl Option<i32> {
    #[pure]
    #[trusted]
    fn is_none(&self) -> bool; //~ ERROR: Invalid external specification
}

// One parameter for both of the target's.
#[extern_spec]
impl<T> Result<T, T> {
    #[trusted]
    fn err(self) -> Option<T>; //~ ERROR: Invalid external specification
}
