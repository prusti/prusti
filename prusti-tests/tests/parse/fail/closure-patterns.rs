#![feature(more_qualified_paths)]
use prusti_contracts::*;

pub struct S {
    pub f: u32,
}

pub struct T(pub u32);

pub trait Tr {
    type A;
    type B;
}

impl Tr for () {
    type A = S;
    type B = T;
}

pub fn ref_binding() {
    let _ = closure!(#[requires(*x > 0)] |ref x: u32| -> u32 { *x }); //~ ERROR: may not contain `ref` bindings
}

pub fn rest() {
    let _ = closure!(#[requires(true)] |S { .. }: S| -> u32 { 0 }); //~ ERROR: may not contain `..`
}

pub fn qualified_struct() {
    let _ = closure!(#[requires(f > 0)] |<() as Tr>::A { f }: S| -> u32 { f }); //~ ERROR: may not contain qualified paths
}

pub fn qualified_tuple_struct() {
    let _ = closure!(#[requires(f > 0)] |<() as Tr>::B(f): T| -> u32 { f }); //~ ERROR: may not contain qualified paths
}
