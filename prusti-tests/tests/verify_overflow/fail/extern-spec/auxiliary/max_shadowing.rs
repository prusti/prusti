pub trait Max {
    fn max(a: i32, b: i32) -> i32;
}

pub struct TestStruct {}

impl TestStruct {
    pub fn max(a: i32, b: i32) -> i32 {
        if a < b {
            b
        } else {
            a
        }
    }
}

impl Max for TestStruct {
    fn max(a: i32, b: i32) -> i32 {
        if a < b {
            b
        } else {
            a
        }
    }
}
