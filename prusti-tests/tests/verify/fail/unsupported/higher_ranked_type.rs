//@ known-failure
#![feature(unsafe_binders)]
#![allow(incomplete_features)]

fn main() {
    let _ = (0..1).filter(|_| true); //~ERROR: higher-ranked lifetimes and types are not supported
}

fn generic<T>() {}

fn unsafe_binder_arg() {
    generic::<unsafe<'a> &'a u32>(); //~ ERROR: cannot encode method body `unsafe_binder_arg`
}

fn conditional_unsafe_binder_arg(x: u32) {
    if x == 0 {
        generic::<unsafe<'a> &'a u32>(); //~ ERROR: cannot encode method body `conditional_unsafe_binder_arg`
    }
}
