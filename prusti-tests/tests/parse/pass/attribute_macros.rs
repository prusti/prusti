use prusti_contracts::*;

#[requires(x > 0 && y > 0)]
#[ensures(false)] // We just want to test the parser, so this should not fail
#[ensures(result >= x && result >= y)]
#[ensures(result == x || result == y)]
pub fn loop_max(x: u32, y: u32) -> u32 {
    let mut r = x;
    while r < y {
        body_invariant!(x <= r && r < y);
        r += 1
    }
    r
}

#[ensures(result == 1)]
#[unsafe(no_mangle)]
pub extern "C" fn unsafe_attribute() -> i32 {
    1
}

#[requires(s != c"abc")]
pub fn c_string_literal(s: &core::ffi::CStr) -> &core::ffi::CStr {
    c"def"
}

#[requires(true)]
pub fn empty_where_clause()
where
{
}
