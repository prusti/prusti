//! A drop contract applies to every value of the type, so it must be generic
//! over all of the type's parameters.

use prusti_contracts::*;

#[extern_spec]
impl Drop for Option<u32> {
    #[requires(matches!(self, None))] //~ ERROR: a `Drop` specification must be generic over all type and const parameters of `std::option::Option`, in order
    fn drop(&mut self);
}

#[extern_spec]
impl<A, B> Drop for Result<B, A> {
    #[requires(false)] //~ ERROR: a `Drop` specification must be generic over all type and const parameters of `std::result::Result`, in order
    fn drop(&mut self);
}
