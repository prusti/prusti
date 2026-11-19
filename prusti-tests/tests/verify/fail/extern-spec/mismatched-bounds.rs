use prusti_contracts::*;
use std::ops::Neg;

// The spec would apply to `Option<T>::is_some` for every `T`, not only
// `T: Copy`.
#[extern_spec]
impl<T: Copy> core::option::Option<T> {
    #[pure]
    #[trusted]
    fn is_some(&self) -> bool; //~ ERROR: Invalid external specification
}

// The same for a bound on the method.
#[extern_spec]
impl<T> core::option::Option<T> {
    #[pure]
    #[trusted]
    fn is_none(&self) -> bool //~ ERROR: Invalid external specification
    where
        T: Copy;
}

// The impl of `Clone` for `Option<T>` only requires `T: Clone`.
#[extern_spec]
impl<T: Clone + Eq> Clone for Option<T> {
    #[trusted]
    fn clone(&self) -> Self; //~ ERROR: Invalid external specification
}

// `Neg` has no supertraits.
#[extern_spec]
trait Neg: Copy {
    #[trusted]
    fn neg(self) -> Self::Output; //~ ERROR: Invalid external specification
}

// `Borrowed` is `?Sized` in the trait.
#[extern_spec(std::borrow)]
trait Borrow<Borrowed> {
    #[pure]
    #[trusted]
    fn borrow(&self) -> &Borrowed; //~ ERROR: Invalid external specification
}

// `T` is `?Sized` in the function.
#[extern_spec(std::mem)] //~ ERROR: Invalid external specification
#[pure]
#[trusted]
fn size_of_val<T>(val: &T) -> usize;

// `Self` is only behind a reference, so it need not be `Sized`.
#[extern_spec(std::fmt)]
trait Display {
    #[trusted]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result //~ ERROR: Invalid external specification
    where
        Self: Sized;
}
