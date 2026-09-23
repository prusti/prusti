use prusti_contracts::*;

// The call to the trait method `AsMut::as_mut` resolves to the impl for
// arrays, whose extern spec talks about the `&mut` result; it cannot be
// stated as an axiom of the trait method's stub.
#[extern_spec]
impl<T, const N: usize> AsMut<[T]> for [T; N] {
    #[trusted]
    #[ensures(result.len() == N)]
    fn as_mut(&mut self) -> &mut [T];
}

fn as_slice(a: &mut [i32; 3]) {
    let s = a.as_mut();
    assert!(s.len() == 3);
}
