//@ known-failure
//@ aux-build: max_point_mut.rs
use max_point_mut::{Max, Point};
use prusti_contracts::*;

#[extern_spec]
impl Max for Point {
    #[trusted]
    #[pure]
    #[ensures(result >= self.0 && result >= self.1)]
    #[ensures(result == self.0 || result == self.1)]
    fn max(&mut self) -> i32; //~ ERROR: pure function parameters must be Copy
}

fn main() {
    let mut ts = Point(3, 2);
    let x = ts.max();
    assert!(x == 3)
}
