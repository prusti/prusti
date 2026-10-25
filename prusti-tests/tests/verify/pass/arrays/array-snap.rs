use prusti_contracts::*;

fn main() {}

#[pure]
#[ensures(result == 1)]
fn foo() -> u32 {
    let a = [0, 1, 2];
    a[1]
}

#[derive(Clone, Copy)]
pub struct A(usize);

#[pure]
fn literal() -> [u32; 3] {
    [10, 20, 30]
}

#[pure]
fn repeated() -> [u32; 4] {
    [7; 4]
}

#[pure]
fn repeated_adt() -> [A; 16] {
    [A(5); 16]
}

#[pure]
fn repeated_generic_len<const N: usize>(x: u32) -> [u32; N] {
    [x; N]
}

#[pure]
fn literal_generic<T: Copy>(x: T, y: T) -> [T; 2] {
    [x, y]
}

#[pure]
fn empty() -> [u32; 0] {
    []
}

#[pure]
fn repeated_empty(x: u32) -> [u32; 0] {
    [x; 0]
}

#[ensures(literal()[0] == 10 && literal()[1] == 20 && literal()[2] == 30)]
#[ensures(repeated()[3] == 7)]
#[ensures(repeated_adt()[15].0 == 5)]
#[ensures(repeated_generic_len::<8>(3)[7] == 3)]
#[ensures(literal_generic(1u8, 2u8)[1] == 2)]
#[ensures(empty() === empty() && repeated_empty(1) === repeated_empty(1))]
fn specs() {}

#[requires(i < 3)]
#[ensures(literal()[i] >= 10 && repeated()[i] == 7)]
fn symbolic_index(i: usize) {}

fn impure() {
    let a = literal();
    assert!(a[1] == 20);
    let b = repeated_adt();
    assert!(b[0].0 == 5);
    assert!(foo() == 1);
}
