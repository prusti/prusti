use std::cmp::Ordering;

pub trait MyOrd {
    fn cmp(&self, other: &Self) -> Ordering;
}
