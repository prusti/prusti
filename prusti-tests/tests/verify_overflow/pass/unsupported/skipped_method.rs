//@ compile-flags: -Pskip_unsupported_features=true
use prusti_contracts::*;

// Borrows in the result that expire separately but depend on a common argument
// are not supported, so neither the function nor its caller is encoded. With
// unsupported features skipped, both are only warnings, and the failure of the
// function is not reported again as an error. The false assertions only pass
// because the bodies are not verified: once the feature is supported, they fail.

pub fn caller() {
    let mut a = 1;
    let mut b = 2;
    let (ra, rb) = two(&mut a, &mut b);
    *ra = 3;
    *rb = 4;
    prusti_assert!(false);
}

pub fn two<'a, 'b, 'c>(x: &'a mut u32, y: &'b mut u32) -> (&'c mut u32, &'b mut u32)
where
    'a: 'c,
    'b: 'c,
{
    prusti_assert!(false);
    (x, y)
}
