use prusti_contracts::*;

fn test_u8_ge_zero(x: u8) {
    assert!(x >= 0);
}

fn test_u32_ge_zero(x: u32) {
    assert!(x >= 0);
}

fn test_u64_ge_zero(x: u64) {
    assert!(x >= 0);
}

fn test_usize_ge_zero(x: usize) {
    assert!(x >= 0);
}

fn test_char_upperbound(x: char) {
    assert!(x as u32 <= 0x10ffffu32);
}

#[pure]
#[ensures(result)]
fn pure1(x: u32) -> bool { x >= 0 }

#[pure]
#[ensures(result)]
fn pure2(x: usize) -> bool { x >= 0 }

#[pure]
#[trusted]
fn get_usize() -> usize {
    unimplemented!();
}

#[pure]
#[trusted]
fn get_u32() -> u32 {
    unimplemented!();
}

#[pure]
#[trusted]
fn get_i32() -> i32 {
    unimplemented!();
}

fn test1() {
    assert!(get_usize() >= 0);
    assert!(get_u32() >= 0);
}

fn test3(x: usize, y: u32, z: i32) {
    assert!(x >= 0);
    assert!(y >= 0);
}

#[pure]
fn check(_x: u32) -> bool { true }

#[ensures(forall(|k: u32| check(k)))]
pub fn test() {}
