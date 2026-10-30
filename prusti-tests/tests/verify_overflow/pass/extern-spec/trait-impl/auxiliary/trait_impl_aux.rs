//@ rustc-env: PRUSTI_SKIP_VERIFICATION=true
pub trait Incrementable {
    fn increment(&mut self);
}

pub struct Number(pub i32);

impl Incrementable for Number {
    fn increment(&mut self) {
        self.0 += 1;
    }
}
