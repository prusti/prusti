use prusti_contracts::*;

// Borrowing a part of an array element at a runtime index, e.g. `&mut a[i].x`.
// Once the borrow's statement is over, the index temporary holds no value, and
// folding the part back into the element needs the element's address, which
// reads the index: the PCG labels a dead index local only in the guide of the
// expansion it indexes, not in the places that contain it. Borrowing the
// element first (`&mut (&mut a[i]).x`) is supported. When this is, move these
// functions to `verify/pass/arrays/`.

pub struct S {
    pub x: u32,
    pub y: u32,
}

#[requires(i < 2)]
fn field_of_local_array(mut a: [S; 2], i: usize) -> [S; 2] {
    let r = &mut a[i].x;
    *r = 5;
    a //~ERROR: cannot encode function body `field_of_local_array`: borrowing a part of an array element at a runtime index is not supported yet
}

#[requires(i < 2)]
fn field_behind_reference(a: &mut [S; 2], i: usize) { //~ERROR: cannot encode function body `field_behind_reference`: borrowing a part of an array element at a runtime index is not supported yet
    let r = &mut a[i].x;
    *r = 5;
}

#[requires(i < s.len())]
fn field_of_slice_element(s: &mut [S], i: usize) { //~ERROR: cannot encode function body `field_of_slice_element`: borrowing a part of an array element at a runtime index is not supported yet
    let r = &mut s[i].x;
    *r = 5;
}

#[requires(i < 2)]
fn shared_field(a: [S; 2], i: usize) -> u32 {
    let r = &a[i].x;
    *r
} //~ERROR: cannot encode function body `shared_field`: borrowing a part of an array element at a runtime index is not supported yet

#[requires(i < 2)]
fn returned_field(a: &mut [S; 2], i: usize) -> &mut u32 {
    &mut a[i].x
} //~ERROR: cannot encode function body `returned_field`: borrowing a part of an array element at a runtime index is not supported yet
