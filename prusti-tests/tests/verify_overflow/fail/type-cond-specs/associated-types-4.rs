//@ aux-build: some_trait_generic.rs
use some_trait_generic::SomeTrait;
use prusti_contracts::*;

trait A {
    type AssocType;
}

struct Foo;
impl A for Foo {
    type AssocType = i32;
}
impl<T> SomeTrait<T> for Foo {
    type AssocType = T;

    fn foo(&self, x: T) -> i32 {
        42
    }
}

#[extern_spec(some_trait_generic)]
trait SomeTrait<T> {
    #[trusted]
    #[refine_spec(where Self: A<AssocType = <Self as SomeTrait<T>>::AssocType>, [
        ensures(result > 0)
    ])]
    fn foo(&self, x: T) -> i32;
}

fn main() {
    let f = Foo;
    let res = f.foo(42 as u32);
    assert!(res > 0); //~ ERROR: might not hold
}
