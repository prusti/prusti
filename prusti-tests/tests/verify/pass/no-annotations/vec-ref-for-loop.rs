//@ known-failure
// Verification fails with an unbacktranslatable
// `invariant.not.established:wand.not.found`.

// checking that a loop over two distinct regions (slice::Iter<'a, &'static str>) encodes.
pub fn count(v: &Vec<&'static str>) -> usize {
    let mut n = 0;
    for _s in v {
        n += 1;
    }
    n
}
