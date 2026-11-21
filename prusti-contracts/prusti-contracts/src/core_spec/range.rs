use crate::*;

use core::{
    ops::{
        Bound, Range, RangeBounds, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive,
    },
    slice::SliceIndex,
};

// The postconditions of the `index` functions below are guarded by exactly the
// condition under which `index` does not panic, so that nothing follows about
// the result of a call that panics.
// TODO: make these conditions preconditions of `index`.
#[extern_spec]
impl<T> SliceIndex<[T]> for Range<usize> {
    #[trusted]
    #[pure]
    #[ensures(self.start <= self.end && self.end <= slice.len() ==> result.len() == self.end - self.start)]
    fn index(self, slice: &[T]) -> &[T];
}

#[extern_spec]
impl<T> SliceIndex<[T]> for RangeFrom<usize> {
    #[trusted]
    #[pure]
    #[ensures(self.start <= slice.len() ==> result.len() == slice.len() - self.start)]
    fn index(self, slice: &[T]) -> &[T];
}

#[extern_spec]
impl<T> SliceIndex<[T]> for RangeTo<usize> {
    #[trusted]
    #[pure]
    #[ensures(self.end <= slice.len() ==> result.len() == self.end)]
    fn index(self, slice: &[T]) -> &[T];
}

#[extern_spec]
impl<T> SliceIndex<[T]> for RangeFull {
    #[trusted]
    #[pure]
    #[ensures(result.len() == slice.len())]
    fn index(self, slice: &[T]) -> &[T];
}

#[extern_spec]
impl<T> SliceIndex<[T]> for RangeToInclusive<usize> {
    #[trusted]
    #[pure]
    #[ensures(self.end < slice.len() ==> result.len() == self.end + 1)]
    fn index(self, slice: &[T]) -> &[T];
}

#[extern_spec]
trait RangeBounds<T: ?Sized> {
    #[trusted]
    #[pure]
    fn start_bound(&self) -> Bound<&T>;

    #[trusted]
    #[pure]
    fn end_bound(&self) -> Bound<&T>;
}

// Iterating over a `RangeInclusive` may set its private `exhausted` flag once
// the range has no items left, after which the range is empty whatever `start`
// and `end` are. Its `end_bound` is `Excluded` exactly when the flag is set.
#[extern_spec]
impl<Idx> RangeInclusive<Idx> {
    #[trusted]
    #[pure]
    #[ensures(*result.start() === start)]
    #[ensures(*result.end() === end)]
    #[ensures(matches!(result.end_bound(), Bound::Included(_)))]
    fn new(start: Idx, end: Idx) -> RangeInclusive<Idx>;

    #[trusted]
    #[pure]
    fn start(&self) -> &Idx;

    #[trusted]
    #[pure]
    fn end(&self) -> &Idx;

    #[trusted]
    #[pure]
    #[ensures(result == (matches!(self.end_bound(), Bound::Excluded(_)) || !(*self.start() <= *self.end())))]
    fn is_empty(&self) -> bool
    where
        Idx: PartialOrd;
}

#[extern_spec]
impl<T> SliceIndex<[T]> for RangeInclusive<usize> {
    // `std` slices with `start..end + 1`, or with `end + 1..end + 1` once the
    // range is exhausted.
    #[trusted]
    #[pure]
    #[ensures(*self.end() < slice.len()
        && (matches!(self.end_bound(), Bound::Excluded(_)) || *self.start() <= *self.end() + 1)
        ==> result.len() == if self.is_empty() { 0 } else { *self.end() + 1 - *self.start() })]
    fn index(self, slice: &[T]) -> &[T];
}
