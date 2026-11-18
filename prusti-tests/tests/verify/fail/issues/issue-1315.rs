// Regression test for prusti/prusti#1315: code after dropping a struct with a reference-typed
// field is encoded, so a false claim there is reported.
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

    pub fn after_drop(&self) {
        let foobar = self.foobar();
        drop(foobar);
        prusti_assert!(false); //~ ERROR: the asserted expression might not hold
    }
}
