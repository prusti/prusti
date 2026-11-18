// Regression test for prusti/prusti#442: the body of an unsafe function writing through a raw
// pointer is encoded, so a false claim after the write is reported.
use prusti_contracts::*;

pub unsafe fn volatile_write_zero(ptr: *mut u8) {
    core::ptr::write_volatile(ptr, 0u8);
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}
