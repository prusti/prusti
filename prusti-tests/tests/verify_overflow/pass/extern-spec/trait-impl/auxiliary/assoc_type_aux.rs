//@ rustc-env: PRUSTI_SKIP_VERIFICATION=true
pub trait Incrementable {
    type Result;
    fn increment(&mut self) -> Self::Result;
}

pub struct Number(pub i32);

impl Incrementable for Number {
    type Result = i32;
    fn increment(&mut self) -> i32 {
        self.0 += 1;
        self.0
    }
}
