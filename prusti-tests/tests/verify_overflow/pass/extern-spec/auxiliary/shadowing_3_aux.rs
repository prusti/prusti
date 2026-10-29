pub trait TestTrait { fn foo() -> i32; }
pub struct TestStruct {}

impl TestStruct {
    pub fn foo() -> i32 {
        42
    }
}

impl TestTrait for TestStruct {
    fn foo() -> i32 {
        24
    }
}
