//@ known-failure
// Verification fails with an unbacktranslatable
// `invariant.not.established:wand.not.found`.

// `slice::Iter<'a, &'static str>` has two regions, which region erasure
// collapses into one; this used to make the encoder panic with an index out of
// bounds.
pub fn count(v: &Vec<&'static str>) -> usize {
    let mut n = 0;
    for _s in v {
        n += 1;
    }
    n
}
