pub trait Max {
    fn max(&mut self) -> i32;
}

pub struct Point(pub i32, pub i32);

impl Max for Point {
    fn max(&mut self) -> i32 {
        if self.0 > self.1 {
            self.0
        } else {
            self.1
        }
    }
}
