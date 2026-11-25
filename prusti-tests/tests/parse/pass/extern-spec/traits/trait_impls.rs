//@ aux-build: extern_assoc_traits.rs
#![feature(negative_impls)]
use extern_assoc_traits::{Dummy, ExternTrait};
use prusti_contracts::*;

#[extern_spec]
impl ExternTrait for Dummy {
    #[trusted]
    #[requires(x > 0)]
    fn foo(&self, x: Self::Arg1Ty, y: Self::Arg2Ty) -> Self::Ret1Ty;

    // A method may be declared `default`, as in a specializing impl.
    #[trusted]
    default fn bar(&self) -> Self::Ret2Ty;
}

// An extern spec may be given for a negative, an `unsafe` or a `default` impl.
#[extern_spec]
impl<T: ?Sized> !Send for *const T {}

#[extern_spec]
unsafe impl<T: ?Sized + Send> Send for std::sync::Mutex<T> {}

#[extern_spec]
default impl Clone for String {}
