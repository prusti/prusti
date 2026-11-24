use prusti_contracts::*;

trait Foo {
    #[ensures(result > 42)]
    fn foo(&self) -> i32;
}

struct Dummy {
    inner: i32,
}

#[refine_trait_spec]
impl Foo for Dummy {
    #[ensures(result > 84)]
    fn foo(&self) -> i32 {
        if self.inner > 84 {
            self.inner
        } else {
            102
        }
    }
}

// A refined method may take its receiver as `mut self`.
trait Consume {
    #[ensures(result > 0)]
    fn consume(self) -> i32
    where
        Self: Sized;
}

#[refine_trait_spec]
impl Consume for Dummy {
    #[ensures(result > 1)]
    fn consume(mut self) -> i32 {
        self.inner = 2;
        self.inner
    }
}

fn main() {
    assert!(Dummy { inner: 0 }.consume() > 1);
    let d = Dummy { inner: 144 };
    let val = d.foo();
    assert!(val > 84);
}

