use prusti_contracts::*;

#[derive(Clone,PartialEq,Eq)]
struct A {
    i: i32,
}

#[pure]
fn get_value(_x: &A) -> i32 {
    _x.i
}

#[ensures(result == 1)]
fn test_eq_in_code(_a: &A, _b: &A) -> i32 {
    if *_a == *_b {
        if get_value(_a) == get_value(_b) {
            1
        } else {
            0
        }
    } else {
        if _a == _b {
            2
        } else {
            1
        }
    }
}

fn test_construct_eq() {
    let _a = A { i: 7 };
    let _b = A { i: 7 };
    if _a == _b {
        if get_value(&_a) == get_value(&_b) {
        } else {
            panic!();
        }
    } else {
        panic!();
    }
}

#[requires(_x == _y)]
#[ensures(result == 2*get_value(_x))]
fn test_eq_propagation(_x: &A, _y: &A) -> i32 {
    get_value(_x) + get_value(_y)
}

#[derive(Clone,PartialEq,Eq)]
struct WithTuple {
    i: i32,
    t: (i32, i32),
}

#[pure]
fn get_tuple_value(_x: &WithTuple) -> i32 {
    _x.i + _x.t.0 + _x.t.1
}

#[requires(_x == _y)]
#[ensures(result == 2*get_tuple_value(_x))]
fn test_tuple_eq_propagation(_x: &WithTuple, _y: &WithTuple) -> i32 {
    get_tuple_value(_x) + get_tuple_value(_y)
}
