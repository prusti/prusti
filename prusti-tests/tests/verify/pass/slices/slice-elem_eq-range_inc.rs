//@ known-failure

#![feature(const_panic)]
#![feature(slice_index_methods)]

use prusti_contracts::*;

// These extern specs duplicate the ones in `prusti-contracts`, which is an
// error. `prusti-contracts` only specifies the length of a subslice; once it
// also specifies its elements, remove these extern specs.
#[extern_spec]
impl<T> std::slice::SliceIndex<[T]> for std::ops::Range<usize> {
    #[trusted]
    #[ensures( result.len() == self.end - self.start )]
    #[ensures( forall(|i: usize| (0 <= i && i < result.len()) ==> result[i] === slice[i+self.start]) )]
    #[pure]
    fn index(self, slice: &[T]) -> &[T];
}

#[extern_spec]
impl<T, I: std::slice::SliceIndex<[T]>> std::ops::Index<I> for [T] {
    #[trusted]
    #[pure]
    #[ensures( result === <I as std::slice::SliceIndex<[T]>>::index(index, self) )]
    fn index(&self, index: I) -> &I::Output;
}

#[requires(a.len() > 6)]
fn slice(a: &[i32]) {
    let s = &a[1..4];
    assert!(s[0] == a[1]);

    /*
    let s = &a[..2];
    assert!(s[1] == a[1]);
    let s = &a[1..];
    assert!(s[2] == a[3]);
    let s = &a[..];
    assert!(s[3] == a[3]);*/

    // let s = &a[1..=4];
    // assert!(s[3] == a[4]);

    /*let s = &a[..=5];
    assert!(s[5] == a[5]);*/
}
