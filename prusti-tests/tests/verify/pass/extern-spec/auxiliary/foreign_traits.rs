use std::cmp::Ordering;

pub trait Compare: Eq + PartialOrd<Self> {
    fn compare(&self, other: &Self) -> Ordering;
}

pub trait Convert {
    fn convert<U: Clone>(&self, u: U) -> bool;
}

pub struct Plain;

impl Convert for Plain {
    fn convert<U>(&self, _u: U) -> bool {
        true
    }
}

pub trait Pair {
    fn pair(&self) -> (u8, Self);
}
