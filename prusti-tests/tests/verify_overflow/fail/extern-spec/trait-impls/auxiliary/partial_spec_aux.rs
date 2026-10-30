//@ rustc-env: PRUSTI_SKIP_VERIFICATION=true
pub trait MyTrait {
    fn foo(&self, x: i32) -> i32;
    fn bar(&self, x: i32) -> i32;
}

pub struct MyStruct;
impl MyTrait for MyStruct {
    fn foo(&self, x: i32) -> i32 {
        x + 10
    }

    fn bar(&self, x: i32) -> i32 {
        42
    }
}
