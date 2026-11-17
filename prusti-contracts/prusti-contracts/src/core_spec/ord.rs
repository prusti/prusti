use crate::*;

use core::cmp::{Ord, Ordering, PartialOrd};

// Specified as `std` implements them. Its `is_*` methods compare the
// discriminant (-1, 0 and 1 for `Less`, `Equal` and `Greater`) against zero.
#[extern_spec]
impl Ordering {
    #[trusted]
    #[pure]
    #[ensures(result == matches!(self, Ordering::Equal))]
    fn is_eq(self) -> bool;

    #[trusted]
    #[pure]
    #[ensures(result == !matches!(self, Ordering::Equal))]
    fn is_ne(self) -> bool;

    #[trusted]
    #[pure]
    #[ensures(result == matches!(self, Ordering::Less))]
    fn is_lt(self) -> bool;

    #[trusted]
    #[pure]
    #[ensures(result == matches!(self, Ordering::Greater))]
    fn is_gt(self) -> bool;

    #[trusted]
    #[pure]
    #[ensures(result == !matches!(self, Ordering::Greater))]
    fn is_le(self) -> bool;

    #[trusted]
    #[pure]
    #[ensures(result == !matches!(self, Ordering::Less))]
    fn is_ge(self) -> bool;

    #[trusted]
    #[pure]
    #[ensures(result === (match self {
        Ordering::Less => Ordering::Greater,
        Ordering::Equal => Ordering::Equal,
        Ordering::Greater => Ordering::Less,
    }))]
    fn reverse(self) -> Ordering;

    #[trusted]
    #[pure]
    #[ensures(result === (match self {
        Ordering::Equal => other,
        _ => self,
    }))]
    fn then(self, other: Ordering) -> Ordering;
}

// The trait requires every impl to be consistent with `PartialEq`, and the
// comparisons to agree with `partial_cmp` as their default bodies do (with
// `Option::is_some_and` inlined).
#[extern_spec]
trait PartialOrd<Rhs> {
    #[trusted]
    #[pure]
    #[ensures(matches!(result, Some(Ordering::Equal)) == self.eq(other))]
    fn partial_cmp(&self, other: &Rhs) -> Option<Ordering>;

    #[trusted]
    #[pure]
    #[ensures(result == match self.partial_cmp(other) { None => false, Some(x) => x.is_lt() })]
    fn lt(&self, other: &Rhs) -> bool;

    #[trusted]
    #[pure]
    #[ensures(result == match self.partial_cmp(other) { None => false, Some(x) => x.is_le() })]
    fn le(&self, other: &Rhs) -> bool;

    #[trusted]
    #[pure]
    #[ensures(result == match self.partial_cmp(other) { None => false, Some(x) => x.is_gt() })]
    fn gt(&self, other: &Rhs) -> bool;

    #[trusted]
    #[pure]
    #[ensures(result == match self.partial_cmp(other) { None => false, Some(x) => x.is_ge() })]
    fn ge(&self, other: &Rhs) -> bool;
}

// The trait requires every impl to agree with `partial_cmp`. As
// `Self: PartialOrd<Self>`, the duality that `PartialOrd` requires of opposite
// impls holds for `cmp` with its arguments swapped. `max`, `min` and `clamp`
// are specified as their default bodies.
#[extern_spec]
trait Ord {
    #[trusted]
    #[pure]
    #[ensures(self.partial_cmp(other) === Some(result))]
    #[ensures(other.cmp(self) === result.reverse())]
    fn cmp(&self, other: &Self) -> Ordering;

    #[trusted]
    #[pure]
    #[ensures(result === (if other < self { self } else { other }))]
    fn max(self, other: Self) -> Self
    where
        Self: Sized;

    #[trusted]
    #[pure]
    #[ensures(result === (if other < self { other } else { self }))]
    fn min(self, other: Self) -> Self
    where
        Self: Sized;

    #[trusted]
    #[pure]
    #[requires(min <= max)]
    #[ensures(result === (if self < min { min } else if self > max { max } else { self }))]
    fn clamp(self, min: Self, max: Self) -> Self
    where
        Self: Sized;
}

/// Relates `PartialOrd` on the primitive types to their built-in comparison,
/// which a direct `a < b` compiles to but a generic specification reaching
/// these impls does not. `partial_cmp` is specified as `std` implements it for
/// the floats, which also describes the other primitive types.
macro_rules! impl_partial_ord_primitive {
    ($($ty:ty),*) => {$(
        #[extern_spec]
        impl PartialOrd for $ty {
            #[trusted]
            #[pure]
            #[ensures(match (*self <= *other, *self >= *other) {
                (false, false) => matches!(result, None),
                (false, true) => matches!(result, Some(Ordering::Greater)),
                (true, false) => matches!(result, Some(Ordering::Less)),
                (true, true) => matches!(result, Some(Ordering::Equal)),
            })]
            fn partial_cmp(&self, other: &$ty) -> Option<Ordering>;

            #[trusted]
            #[pure]
            #[ensures(result == (*self < *other))]
            fn lt(&self, other: &$ty) -> bool;

            #[trusted]
            #[pure]
            #[ensures(result == (*self <= *other))]
            fn le(&self, other: &$ty) -> bool;

            #[trusted]
            #[pure]
            #[ensures(result == (*self > *other))]
            fn gt(&self, other: &$ty) -> bool;

            #[trusted]
            #[pure]
            #[ensures(result == (*self >= *other))]
            fn ge(&self, other: &$ty) -> bool;
        }
    )*};
}

impl_partial_ord_primitive!(
    bool, char, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64
);
