use prusti_contracts::*;

trait Foo {
    type Type;

    #[pure]
    fn foo(&self) -> Self::Type;
}

struct Bar;
#[refine_trait_spec]
impl Foo for Bar {
    type Type = i32;

    #[pure]
    fn foo(&self) -> Self::Type {
        42
    }
}

struct Baz<T: Copy>(std::marker::PhantomData<T>);
#[refine_trait_spec]
impl<T: Copy> Foo for Baz<T> {
    type Type = T;

    #[pure]
    #[trusted]
    fn foo(&self) -> Self::Type {
        unimplemented!()
    }
}

trait FooWithDefault {
    type Type: Copy;

    #[trusted]
    #[pure]
    fn foo(&self) -> Self::Type {
        unimplemented!()
    }
}
