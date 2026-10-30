use prusti_contracts::*;
use std::slice::Iter;

#[derive(Copy, Clone)]
pub struct S1<T> {
    f: T,
}

pub fn test1<T: Copy>(a: T) -> T {
    a
}

pub fn test2<T: Copy>(a: T) -> T {
    let b = a;
    b
}

pub fn test3<T: Copy>(a: S1<T>) -> S1<T> {
    a
}

pub fn test4<T: Copy>(a: S1<T>) -> S1<T> {
    let b = a;
    b
}

// Checking whether the params of pure methods are copy or not

#[pure]
#[trusted]
fn foo<'a, T: Copy>(val: T) -> Option<&'a T> { unimplemented!() }

#[pure]
#[trusted]
fn bar<'a, T: Copy>(val: T) -> &'a Option<&'a T> { unimplemented!() }

#[pure]
#[trusted]
fn baz<'a, T>(val: &'a T) -> Option<&'a T> { unimplemented!() }

#[model]
struct Iter<'a, #[generic] T: Copy> {
    position: usize,
}

type SliceTy<T> = [T];
#[extern_spec]
impl<T: Copy+PartialEq> SliceTy<T> {
    #[trusted]
    #[ensures( result.model().position == 0 )]
    fn iter(&self) -> std::slice::Iter<'_, T>;
}

fn verify_slice_iter(slice: &[i32]) {
    let iter = slice.iter();
}
