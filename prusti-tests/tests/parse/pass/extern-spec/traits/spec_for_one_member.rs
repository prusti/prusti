//@ aux-build: extern_traits.rs
use extern_traits::ExternTrait;
use prusti_contracts::*;

#[extern_spec(extern_traits)]
trait ExternTrait {
    #[trusted]
    #[ensures(result == 42)]
    fn foo(&self) -> i32;
}

struct S;

impl ExternTrait for S {
    fn foo(&self) -> i32 {
        42
    }

    fn bar(&self) -> i32 {
        43
    }
}

fn main() {
    let s = S {};
    assert!(s.foo() == 42);
}