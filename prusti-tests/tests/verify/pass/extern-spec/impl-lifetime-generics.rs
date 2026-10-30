use prusti_contracts::*;

// Extern specs on foreign impls whose self type has a lifetime parameter. The
// spec items have the generics written in the extern spec, so `T` is at a
// different position than in the specified method whenever the lifetimes
// differ: here `'a` precedes `T` like in the target ...
#[extern_spec]
impl<'a, T> core::slice::Iter<'a, T> {
    #[pure]
    #[trusted]
    #[ensures(result.len() <= usize::MAX)]
    fn as_slice(&self) -> &'a [T];
}

// ... while an elided lifetime in the impl header follows `T`.
#[extern_spec]
impl<T> core::slice::IterMut<'_, T> {
    #[pure]
    #[trusted]
    #[ensures(result.len() <= usize::MAX)]
    fn as_slice<'b>(&'b self) -> &'b [T];
}

fn use_as_slice(v: &[i32]) {
    let it = v.iter();
    let s = it.as_slice();
    assert!(s.len() <= usize::MAX);
}

fn use_as_slice_mut(v: &mut [i32]) {
    let it = v.iter_mut();
    let s = it.as_slice();
    assert!(s.len() <= usize::MAX);
}
