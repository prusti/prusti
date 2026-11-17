use prusti_contracts::*;

#[requires(a.len() > 5)]
fn slice(a: &[i32]) {
    let s = &a[1..4];
    assert!(s.len() == 3);
    let s = &a[..2];
    assert!(s.len() == 2);
    let s = &a[1..];
    assert!(s.len() == a.len()-1);
    let s = &a[..];
    assert!(s.len() == a.len());
    let s = &a[1..=4];
    assert!(s.len() == 4);
    let s = &a[2..=1];
    assert!(s.len() == 0);
    let s = &a[..=4];
    assert!(s.len() == 5);
}

#[requires(!r.is_empty() && *r.end() < a.len())]
fn slice_inclusive(a: &[i32], r: std::ops::RangeInclusive<usize>) {
    let n = *r.end() + 1 - *r.start();
    let s = &a[r];
    assert!(s.len() == n);
}
