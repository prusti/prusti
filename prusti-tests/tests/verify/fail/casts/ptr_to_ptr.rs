// A cast to a thin pointer drops the metadata, so wide pointers with the same
// address but different metadata have equal thin casts.

#![allow(ambiguous_wide_pointer_comparisons)]

fn thin_eq_implies_wide_eq(a: *const [u8], b: *const [u8]) {
    if a as *const u8 == b as *const u8 {
        assert!(a == b); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
    }
}

fn wide_ne_implies_thin_ne(a: *const [u8], b: *const [u8]) {
    if a != b {
        assert!(a as *const u8 != b as *const u8); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
    }
}

fn mut_wide_ne_implies_thin_ne(a: *mut [u8], b: *mut [u8]) {
    if a != b {
        assert!(a as *mut u8 != b as *mut u8); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
    }
}

fn generic_wide_ne_implies_thin_ne<T: ?Sized>(a: *const T, b: *const T) {
    if a != b {
        assert!(a as *const u8 != b as *const u8); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
    }
}
