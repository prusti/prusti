use prusti_contracts::*;
use core::cmp::Ordering;

#[ensures(matches!(a.partial_cmp(b), Some(Ordering::Equal)) == (*a == *b))]
#[ensures((*a < *b) == matches!(a.partial_cmp(b), Some(Ordering::Less)))]
#[ensures((*a > *b) == matches!(a.partial_cmp(b), Some(Ordering::Greater)))]
#[ensures((*a <= *b) == (*a < *b || *a == *b))]
#[ensures((*a >= *b) == (*a > *b || *a == *b))]
fn partial_ord_consistent<T: PartialOrd>(a: &T, b: &T) {}

#[ensures(a.partial_cmp(b) === Some(a.cmp(b)))]
#[ensures((*a < *b) == matches!(a.cmp(b), Ordering::Less))]
#[ensures(*a < *b || *a == *b || *a > *b)]
#[ensures(matches!(a.cmp(b), Ordering::Less) == matches!(b.cmp(a), Ordering::Greater))]
#[ensures(matches!(a.cmp(a), Ordering::Equal))]
fn ord_consistent<T: Ord>(a: &T, b: &T) {}

#[ensures(result === (if matches!(a.cmp(&b), Ordering::Greater) { a } else { b }))]
fn max<T: Ord + Copy>(a: T, b: T) -> T {
    a.max(b)
}

#[ensures(result === (if matches!(a.cmp(&b), Ordering::Greater) { b } else { a }))]
fn min<T: Ord + Copy>(a: T, b: T) -> T {
    a.min(b)
}

#[pure]
fn le<T: PartialOrd>(a: &T, b: &T) -> bool {
    *a <= *b
}

fn primitives() {
    assert!(le(&1usize, &4usize));
    assert!(!le(&5usize, &4usize));
    assert!(le(&-3i32, &-3i32));
    assert!(le(&false, &true));
    assert!(le(&'a', &'b'));
    assert!(le(&1.0f64, &2.0f64));
}

fn primitive_ord(x: i32, y: i32) {
    assert!(matches!(3i32.cmp(&5), Ordering::Less));
    assert!(matches!(5i32.partial_cmp(&3), Some(Ordering::Greater)));
    assert!(3i32.min(5) == 3);
    assert!(3i32.max(5) == 5);
    assert!(7i32.clamp(1, 5) == 5);
    if x < y {
        assert!(matches!(x.cmp(&y), Ordering::Less));
    }
}

fn ordering_methods() {
    assert!(Ordering::Less.is_lt() && Ordering::Less.is_le() && !Ordering::Less.is_gt());
    assert!(Ordering::Equal.is_eq() && Ordering::Equal.is_ge() && !Ordering::Equal.is_ne());
    assert!(matches!(Ordering::Greater.reverse(), Ordering::Less));
    assert!(matches!(Ordering::Equal.then(Ordering::Less), Ordering::Less));
    assert!(matches!(Ordering::Greater.then(Ordering::Less), Ordering::Greater));
}

// An impl in the common lexicographic style, which has to satisfy the specs of
// the traits.
#[derive(PartialEq, Eq)]
struct Pair {
    a: u32,
    b: u32,
}

impl PartialOrd for Pair {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Pair {
    fn cmp(&self, other: &Self) -> Ordering {
        self.a.cmp(&other.a).then(self.b.cmp(&other.b))
    }
}

fn pair() {
    let p = Pair { a: 1, b: 5 };
    let q = Pair { a: 2, b: 0 };
    assert!(p < q);
}
