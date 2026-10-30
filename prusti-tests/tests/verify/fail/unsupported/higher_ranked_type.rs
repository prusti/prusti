//@ known-failure
#![feature(unsafe_binders)]
#![allow(incomplete_features)]

fn generic<T>() {}

fn unsafe_binder_arg() {
    generic::<unsafe<'a> &'a u32>(); //~ ERROR: unsafe binder types are not supported
}

fn conditional_unsafe_binder_arg(x: u32) {
    if x == 0 {
        generic::<unsafe<'a> &'a u32>(); //~ ERROR: unsafe binder types are not supported
    }
}
