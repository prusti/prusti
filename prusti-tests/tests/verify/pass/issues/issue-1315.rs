// Regression test for prusti/prusti#1315: dropping structs with reference-typed fields.
use prusti_contracts::*;

pub struct Foo<T> {
    foo: T,
}
pub struct Bar<'a> {
    bar: &'a (),
}

pub struct Baz;

impl Baz {
    #[trusted]
    pub fn foobar(&self) -> Foo<Bar<'_>> {
        unimplemented!()
    }

    pub fn baz(&self) {
        let foobar = self.foobar();
        drop(foobar);
    }
}

pub struct WithDrop<'a> {
    foo: &'a (),
}

impl<'a> Drop for WithDrop<'a> {
    fn drop(&mut self) {}
}

impl Baz {
    #[trusted]
    pub fn with_drop(&self) -> WithDrop<'_> {
        unimplemented!()
    }

    pub fn explicit_drop(&self) {
        let w = self.with_drop();
        drop(w);
    }

    pub fn implicit_drop(&self) {
        let _w = self.with_drop();
    }
}
