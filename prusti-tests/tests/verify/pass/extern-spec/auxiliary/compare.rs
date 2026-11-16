use std::cmp::Ordering;

pub trait Compare: Eq + PartialOrd<Self> {
    fn compare(&self, other: &Self) -> Ordering;
}
