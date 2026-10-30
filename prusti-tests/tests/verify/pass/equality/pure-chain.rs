use prusti_contracts::*;

#[derive(Clone,Copy,PartialEq,Eq)]
struct A {
    i: i32,
}

#[pure]
fn pos(_x: A) -> bool {
    _x.i > 0
}

#[pure]
#[requires(pos(_x) && pos(_y))]
#[ensures(pos(result))]
fn first_pos(_x: A, _y: A) -> A {
    _x
}

#[pure]
#[requires(pos(_x) && pos(_y))]
#[ensures(pos(result) && pos(_x) && pos(_y))]
fn first_pos_all(_x: A, _y: A) -> A {
    _x
}

#[pure]
fn pos_above(_x: A, _j: i32) -> bool {
    _x.i > _j
}

#[pure]
#[requires(pos_above(_x, 17) && pos_above(_y, 17))]
#[ensures(pos_above(result, 17) && pos_above(_x, 17) && pos_above(_y, 17))]
fn first_pos_above(_x: A, _y: A) -> A {
    _x
}

#[pure]
fn id(_x: A) -> A {
    _x
}

#[pure]
#[requires(_x == _y)]
#[ensures(result == id(_y))]
fn first_eq_id(_x: A, _y: A) -> A {
    _x
}

#[pure]
#[ensures(result == x)]
fn id_spec(x: A) -> A {
    x
}

#[pure]
#[ensures(result == id_spec(id_spec(id_spec(id_spec(x)))))]
fn nested_id(x: A) -> A {
    x
}

#[pure]
#[requires(_x == _y)]
#[ensures(id(result) == id(_y))]
fn first_id_eq_id(_x: A, _y: A) -> A {
    _x
}

#[pure]
#[requires(id(_x) == id(_y))]
fn first_id_pre(_x: A, _y: A) -> A {
    _x
}

// Impure functions with pure calls in their contracts.

#[requires(_x == _y)]
#[ensures(id(result) == id(_y))]
fn impure_first_id_eq_id(_x: A, _y: A) -> A {
    _x
}

#[requires(id(_x) == id(_y))]
#[ensures(result == _y)]
fn impure_first_id_pre(_x: A, _y: A) -> A {
    _x
}
