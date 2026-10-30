use prusti_contracts::*;

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
    let c = [10, 20, 30];
    prusti_assert!(c === literal());
    let b = repeated_adt();
    assert!(b[0].0 == 5);
    assert!(foo() == 1);
}

#[pure]
#[ensures(result == a[0])]
fn first(a: &[i32; 5]) -> i32 {
    a[0]
}

#[pure]
#[ensures(result == a.len())]
#[ensures(result == 17)]
fn array_foo(a: &[i32; 17]) -> usize {
    a.len()
}

// FIXME: predicate here
#[pure]
fn sorted_ish(a: &[i32; 4]) -> bool {
    a[0] <= a[1]
        && a[1] <= a[2]
        && a[2] <= a[3]
}

fn do_it() {
    let a = [0, 1, 2, 3];

    assert!(sorted_ish(&a));
}

struct Foo {
    x: usize,
    y: usize,
}

// This testcase (also) tests that in function name mangling, if the element type of a sequence is
// itself a snapshot (like for the array snapshot constructor for `f` here) we do the recursive
// call to encode the element type's name as well. if we didn't, it would come out as something
// like Seq$Snapshot(elem_ty) and later stages will choke on the parentheses.
#[pure]
fn struct_array_sum(f: [Foo; 1]) -> usize {
    f[0].x + f[0].y
}
