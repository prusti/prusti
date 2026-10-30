use prusti_contracts::*;

#[derive(Clone,Copy,PartialEq,Eq)]
struct A {
    i: i32,
}

#[pure]
#[requires(_x == _y)]
fn get_value_pre(_x: A, _y: A) -> A {
    _x
}

#[pure]
#[requires(_x == _y)]
#[ensures(_x == _y)]
fn get_value(_x: A, _y: A) -> A {
    _x
}

#[pure]
#[ensures(result == _x)]
fn get_value_result_eq(_x: A, _y: A) -> A {
    _x
}

#[pure]
#[ensures(_x == result)]
fn get_value_eq_result(_x: A, _y: A) -> A {
    _x
}

#[pure]
#[requires(_x != _y)]
#[ensures(result != _y)]
fn first(_x: A, _y: A) -> A {
    _x
}

#[derive(Clone,PartialEq,Eq)]
struct B {
    i: i32,
}

#[pure]
#[requires(_x == _y)]
#[ensures(old(_x) == old(_y))]
fn get_value_non_copy(_x: B, _y: B) -> B {
    _x
}

#[requires(_x == _y)]
#[ensures(old(_x) == old(_y))]
fn get_value_old(_x: B, _y: B) -> B {
    _x
}
