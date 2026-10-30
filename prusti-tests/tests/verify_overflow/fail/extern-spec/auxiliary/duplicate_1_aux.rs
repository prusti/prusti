use prusti_contracts::*;

pub struct TestStruct {}

impl TestStruct {
    #[pure]
    #[ensures(result >= a && result >= b)]
    #[ensures(result == a || result == b)]
    pub fn max(a: i32, b: i32) -> i32 {
        if a < b {
            b
        } else {
            a
        }
    }
}
