use prusti_contracts::*;

trait A {
    #[pure]
    fn res(&self) -> i32 { 0 }
}

struct Foo;

#[refine_trait_spec]
impl A for Foo {
    #[pure]
    #[ensures(result == 42)]
    fn res(&self) -> i32 { 42 }
}

fn verify(foo: Foo) {
    assert!(foo.res() == 42);
}

trait Trait<Tp> {
    #[ensures(true)]
    fn fn1<Tx, Ty, Tz>();
}

struct Struct<Ex, Ey> { x: Ex, y: Ey }

impl<A, B, C> Trait<A> for Struct<B, C> {
    // inherit spec
    fn fn1<X, Y, Z>() {}
}

trait Validity {
    #[pure]
    fn valid(&self) -> bool;
}

#[requires(a.valid())]
fn test<T: Validity>(a: T) -> T {
    a
}
