//@ aux-build: duplicate_1_aux.rs
use duplicate_1_aux::MyTrait;
use prusti_contracts::*;

#[extern_spec(duplicate_1_aux)]
trait MyTrait {
    #[trusted]
    #[requires(true)]
    fn foo(&mut self) -> i32; //~ ERROR: duplicate specification for `duplicate_1_aux::MyTrait::foo`
}
