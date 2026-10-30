//@ aux-build: extern_assoc_traits.rs
use extern_assoc_traits::{Dummy, ExternTrait};
use prusti_contracts::*;

#[extern_spec]
impl ExternTrait for Dummy {
    #[trusted]
    #[requires(x > 0)]
    fn foo(&self, x: Self::Arg1Ty, y: Self::Arg2Ty) -> Self::Ret1Ty;
}
