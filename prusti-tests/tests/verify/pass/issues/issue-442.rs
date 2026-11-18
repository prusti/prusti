// Regression test for prusti/prusti#442: unsafe functions taking and writing through a raw
// pointer must not crash the encoder.
pub unsafe fn volatile_write_zero(ptr: *mut u8) {
    core::ptr::write_volatile(ptr, 0u8);
}

pub unsafe fn take_ptr(_ptr: *mut u8) {}
