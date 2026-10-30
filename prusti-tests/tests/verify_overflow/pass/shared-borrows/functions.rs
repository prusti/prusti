use prusti_contracts::*;

fn borrow(_x: &u32) {}

fn borrow2(_x: &u32, _y: &u32) {}

pub fn test1() {
    let a = 5;
    let x = &a;
    borrow(x);
}

pub fn test2(x: &u32) {
    let a = *x;
    let y = &a;
    borrow(y);
}

pub fn test3(x: &u32) {
    borrow(x);
}

pub fn test4(x: &mut u32) {
    borrow(x);
}

pub fn test5() {
    let a = 5;
    let b = 6;
    let x = &a;
    let y = &b;
    borrow2(x, y);
}

#[requires(*x == 5)]
#[ensures(*x == 5)]
pub fn test6(x: &u32) {}

#[ensures(*x == old(*x))]
pub fn test7(x: &u32) {}

pub fn test8() {
    let a = 5;
    borrow(&a);
    assert!(a == 5);
    let x = &a;
    borrow(&a);
    borrow2(x, &a);
    assert!(a == 5);
    assert!(*x == 5);
}

pub fn test_static<T>(x: &'static T) -> &'_ T {
    &*x
}

struct T<'a> {
    f: u32,
    g: &'a u32,
    h: *const u32,
    i: Vec<u32>,
    j: String,
    k: &'a [T<'a>],
}

#[requires(x.f == 123)]
#[ensures(result == 123)]
fn test_unused_ref_fields(x: &T) -> u32 {
    x.f
}
