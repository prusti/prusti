//@ aux-build: extern_assoc_traits.rs
use extern_assoc_traits::ExternTrait;
use prusti_contracts::*;

#[extern_spec(extern_assoc_traits)]
trait ExternTrait {
    #[trusted]
    fn foo(&self, x: Self::Arg1Ty, y: Self::Arg2Ty) -> Self::Ret1Ty;
}
