//@ aux-build: max_point.rs
use max_point::{Max, Point};
use prusti_contracts::*;

#[extern_spec]
impl Max for Point {
    #[trusted]
    #[pure]
    #[ensures(result >= self.0 && result >= self.1)]
    #[ensures(result == self.0 || result == self.1)]
    fn max(&self) -> i32;
}

fn main() {
    let ts = Point(3, 2);
    let x = ts.max();
    assert!(x == 3)
}
