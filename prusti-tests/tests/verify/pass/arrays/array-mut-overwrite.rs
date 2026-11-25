//@ known-failure
use prusti_contracts::*;

// The other element keeps its reference after one is replaced.
#[ensures(*result[0] == 4)]
fn write_element<'a>(mut y: [&'a mut i32; 2], r: &'a mut i32) -> [&'a mut i32; 2] {
    y[1] = r;
    *y[0] = 4;
    y
}
