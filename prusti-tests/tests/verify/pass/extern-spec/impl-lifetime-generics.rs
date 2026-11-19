use prusti_contracts::*;

// Extern specs on foreign impls with a lifetime parameter before `T`, written
// out and elided in the impl header. The specs are trivially true; what is
// tested is that their items are instantiated with the target's `T` (this used
// to panic).
#[extern_spec]
impl<'a, T> core::slice::Iter<'a, T> {
    #[pure]
    #[trusted]
    #[ensures(result.len() <= usize::MAX)]
    fn as_slice(&self) -> &'a [T];
}

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
