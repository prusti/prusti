//@ known-failure
//@ aux-build: some_trait.rs
use some_trait::SomeTrait;
use prusti_contracts::*;

trait A {
    type AssocType;
}

struct Foo<T> {
    x: T
}
impl<T> A for Foo<T> {
    type AssocType = i32;
}
impl<T> SomeTrait for Foo<T> {
    type AssocType = T;

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
    let f = Foo {
        x: 42 as i32
    };
    let res = f.foo();
    assert!(res > 0);
}
