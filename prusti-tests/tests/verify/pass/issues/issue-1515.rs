// Regression test for prusti/prusti#1515: a trusted extern spec on a trait impl with const
// generics resolves to the concrete type and is used at the call.
extern crate prusti_contracts;
use prusti_contracts::*;

#[extern_spec]
impl<T, const N: usize> std::convert::TryFrom<&[T]> for [T; N]
where
    T: Copy,
{
    #[trusted]
    #[ensures(slice.len() == N ==> result.is_ok())]
    fn try_from(slice: &[T]) -> Result<[T; N], std::array::TryFromSliceError>;
}

#[requires(a.len() == 4)]
fn parse_u32(a: &[u8]) -> u32 {
    let bytes: Result<[u8; 4], _> = std::convert::TryFrom::try_from(a);
    match bytes {
        Ok(x) => u32::from_le_bytes(x),
        Err(_) => {
            prusti_assert!(false);
            0
        }
    }
}
