use prusti_contracts::*;

extern "C" {
    fn max(a: i32, b: i32) -> i32;
}

unsafe extern "C" {
    fn min(a: i32, b: i32) -> i32;
    safe fn getpid() -> i32;
    unsafe fn llabs(a: i64) -> i64;
}

#[extern_spec(crate)]
extern "C" {
    #[trusted]
    #[ensures(a >= b ==> result == a)]
    #[ensures(b >= a ==> result == b)]
    fn max(a: i32, b: i32) -> i32;
}

#[extern_spec(crate)]
unsafe extern "C" {
    #[trusted]
    #[ensures(a <= b ==> result == a)]
    #[ensures(b <= a ==> result == b)]
    fn min(a: i32, b: i32) -> i32;

    #[trusted]
    #[ensures(result > 0)]
    safe fn getpid() -> i32;

    #[trusted]
    #[ensures(result >= 0)]
    unsafe fn llabs(a: i64) -> i64;
}

fn test() {
    assert!(unsafe { max(1, 2) } == 2);
    assert!(unsafe { min(1, 2) } == 1);
    assert!(getpid() > 0);
    assert!(unsafe { llabs(-1) } >= 0);
}
