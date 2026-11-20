use prusti_contracts::*;

// A pledge on a result with borrows that expire separately is not supported.
// It is reported at the pledge and left out: the function is still encoded
// and verified without it.

#[after_expiry(true)] //~ERROR: pledges on a function whose result contains borrows that expire separately are not supported
pub fn two<'a, 'b>(x: &'a mut u32, y: &'b mut u32) -> (&'a mut u32, &'b mut u32) {
    prusti_assert!(false); //~ERROR: the asserted expression might not hold
    (x, y)
}

// The pledge would not hold, but it is left out, so only the unsupported
// feature is reported.
#[after_expiry(false)] //~ERROR: pledges on a function whose result contains borrows that expire separately are not supported
pub fn two_false<'a, 'b>(x: &'a mut u32, y: &'b mut u32) -> (&'a mut u32, &'b mut u32) {
    (x, y)
}
