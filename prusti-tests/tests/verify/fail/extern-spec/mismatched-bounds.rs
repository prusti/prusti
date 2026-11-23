use prusti_contracts::*;
use std::ops::Neg;

// The spec would apply to `Option<T>::is_some` for every `T`, not only
// `T: Copy`. The bound is reported once, not for each method.
#[extern_spec]
impl<T: Copy> core::option::Option<T> { //~ ERROR: Invalid external specification
    #[pure]
    #[trusted]
    fn is_some(&self) -> bool;

    #[trusted]
    fn as_ref(&self) -> Option<&T>;
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
impl<T: Clone + Eq> Clone for Option<T> { //~ ERROR: Invalid external specification
    #[trusted]
    fn clone(&self) -> Self;
}

// `Neg` has no supertraits.
#[extern_spec]
trait Neg: Copy { //~ ERROR: Invalid external specification
    #[trusted]
    fn neg(self) -> Self::Output
    where
        Self: Sized;
}

// `Borrowed` is `?Sized` in the trait.
#[extern_spec(std::borrow)]
trait Borrow<Borrowed> { //~ ERROR: Invalid external specification
    #[pure]
    #[trusted]
    fn borrow(&self) -> &Borrowed;
}

// `T` is `?Sized` in the function.
#[extern_spec(std::mem)]
#[pure]
#[trusted]
fn size_of_val<T>(val: &T) -> usize; //~ ERROR: Invalid external specification

// `Self` is only behind a reference, so it need not be `Sized`.
#[extern_spec(std::fmt)]
trait Display {
    #[trusted]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result //~ ERROR: Invalid external specification
    where
        Self: Sized;
}

// An inline bound of a trait parameter with a default.
#[extern_spec(std::ops)]
trait AddAssign<Rhs: Copy = Self> { //~ ERROR: Invalid external specification
    #[trusted]
    fn add_assign(&mut self, rhs: Rhs);
}
