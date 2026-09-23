#![feature(const_convert, const_trait_impl)]

use prusti_contracts::*;

const ONE: Real = Real::from(1isize);
const TWO: Real = Real::from(2isize);

#[pure]
fn three() -> Real {
    Real::from(3isize)
}

#[ensures(ONE + ONE == TWO)]
#[ensures(ONE / TWO + ONE / TWO == ONE)]
#[ensures(TWO + ONE == three())]
pub fn constants() {}

#[requires(-100 < x && x < 100 && -100 < y && y < 100)]
#[ensures(Real::from(x) + Real::from(y) == Real::from(x + y))]
#[ensures(x < y ==> Real::from(x) < Real::from(y))]
pub fn symbolic(x: isize, y: isize) {}
