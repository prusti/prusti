pub trait Trait {
    fn foo(&self) -> i32;
}

pub struct Struct1;
pub struct Struct2;

impl Trait for Struct1 {
    fn foo(&self) -> i32 {
        42
    }
}

impl Trait for Struct2 {
    fn foo(&self) -> i32 {
        43
    }
}
