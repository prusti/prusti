// Coverage for the `*mut T` to `*const T` pointer coercion, which keeps the
// address and the metadata of the pointer.

use prusti_contracts::*;

fn implicit(p: *mut i32) -> *const i32 {
    p
}

fn explicit(p: *mut i32) -> *const i32 {
    p as *const i32
}

fn null() -> *const i32 {
    std::ptr::null_mut()
}

fn slice(p: *mut [i32]) -> *const [i32] {
    p
}

#[pure]
fn pure_implicit(p: *mut u8) -> *const u8 {
    p
}

#[ensures(result == pure_implicit(p))]
fn same_as_pure(p: *mut u8) -> *const u8 {
    p
}
