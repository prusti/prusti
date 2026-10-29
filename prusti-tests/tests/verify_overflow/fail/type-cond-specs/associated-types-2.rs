//@ aux-build: some_trait.rs
use some_trait::SomeTrait;
use prusti_contracts::*;

trait A {
    type AssocType;
}

struct Foo;
impl A for Foo {
    type AssocType = i32;
}
impl SomeTrait for Foo {
    type AssocType = u32;

    fn foo(&self) -> i32 {
        42
    }
}

#[extern_spec(some_trait)]
trait SomeTrait {
    #[trusted]
    #[refine_spec(where Self: A<AssocType = <Self as SomeTrait>::AssocType>, [
        ensures(result > 0)
    ])]
    fn foo(&self) -> i32;
}

fn main() {
    let f = Foo;
    let res = f.foo();
    assert!(res > 0); //~ ERROR: might not hold
}
