fn test_panic(x: bool) {
    panic!();  //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn test_assert(x: bool) {
    assert!(false);  //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn test_assert_msg(x: bool) {
    assert!(false, "msg");  //~ERROR: precondition of `std::rt::panic_fmt` (called by this macro expansion) might not hold
}

fn test_debug_assert(x: bool) {
    debug_assert!(false);  //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn test_debug_assert_msg(x: bool) {
    debug_assert!(false, "msg");  //~ERROR: precondition of `std::rt::panic_fmt` (called by this macro expansion) might not hold
}

fn test_unreachable(x: bool) {
    unreachable!();  //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn test_unimplemented(x: bool) {
    unimplemented!();  //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn test_assert_matches(x: bool) {
    std::assert_matches!(None::<i32>, Some(_));  //~ERROR: precondition of `core::panicking::assert_matches_failed` (called by this macro expansion) might not hold
}

fn test_panic_any(x: bool) {
    std::panic::panic_any(5);  //~ERROR: precondition might not hold
}

macro_rules! inner_panic {
    () => {
        {
            panic!();
        }
    };
}

fn test_inner_panic(x: bool) {
    inner_panic!();  //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

macro_rules! inner_panic_msg {
    () => {
        {
            panic!("msg");
        }
    };
}

fn test_inner_panic_msg(x: bool) {
    inner_panic_msg!();  //~ERROR: precondition of `std::rt::panic_fmt` (called by this macro expansion) might not hold
}

macro_rules! inner_assert {
    () => {
        {
            assert!(false);
        }
    };
}

fn test_inner_assert(x: bool) {
    inner_assert!();  //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

macro_rules! inner_assert_msg {
    () => {
        {
            assert!(false, "msg");
        }
    };
}

fn test_inner_assert_msg(x: bool) {
    inner_assert_msg!();  //~ERROR: precondition of `std::rt::panic_fmt` (called by this macro expansion) might not hold
}

macro_rules! inner_debug_assert {
    () => {
        {
            debug_assert!(false);
        }
    };
}

fn test_inner_debug_assert(x: bool) {
    inner_debug_assert!();  //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

macro_rules! inner_debug_assert_msg {
    () => {
        {
            debug_assert!(false, "msg");
        }
    };
}

fn test_inner_debug_assert_msg(x: bool) {
    inner_debug_assert_msg!();  //~ERROR: precondition of `std::rt::panic_fmt` (called by this macro expansion) might not hold
}

macro_rules! inner_unreachable {
    () => {
        {
            unreachable!();
        }
    };
}

fn test_inner_unreachable(x: bool) {
    inner_unreachable!();  //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

macro_rules! inner_unimplemented {
    () => {
        {
            unimplemented!();
        }
    };
}

fn test_inner_unimplemented(x: bool) {
    inner_unimplemented!();  //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}
