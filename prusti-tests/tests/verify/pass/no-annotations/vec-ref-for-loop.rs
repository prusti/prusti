//@ known-failure
// Checks that a loop over an iterator with two distinct regions
// (slice::Iter<'a, &'static str>) encodes.
// Verification fails with an unbacktranslatable
// `invariant.not.established:wand.not.found`.

pub fn count(v: &Vec<&'static str>) -> usize {
    let mut n = 0;
    for _s in v {
        n += 1;
    }
    n
}
