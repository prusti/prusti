use prusti_contracts::*;

#[derive(Clone,PartialEq,Eq)]
enum A {
    ANone(u32),
    ASome(i32),
}

#[requires(_x == _y)]
#[ensures(result == 17)]
fn test(_x: &A, _y: &A) -> i32 {
    17
}

#[derive(Clone,PartialEq,Eq)]
enum Empty {
}

#[requires(_x == _y)]
#[ensures(result == 17)]
fn test_empty(_x: &Empty, _y: &Empty) -> i32 {
    17
}

#[derive(Clone,PartialEq,Eq)]
enum NoFields {
    U,
    V,
}

#[requires(_x == _y)]
#[ensures(result == 17)]
fn test_no_fields(_x: &NoFields, _y: &NoFields) -> i32 {
    17
}

#[derive(Clone,PartialEq,Eq)]
enum OneVariant {
    U(u32),
}

#[requires(_x == _y)]
#[ensures(result == 17)]
fn test_one_variant(_x: &OneVariant, _y: &OneVariant) -> i32 {
    17
}
