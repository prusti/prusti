pub trait Trait {
    fn foo(&mut self) -> i32 { 3 }
}

pub struct Test {}

impl Trait for Test {
    fn foo(&mut self) -> i32 { 5 }
}
