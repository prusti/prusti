// Refinements whose bounds the call site does not satisfy are not applied.
use prusti_contracts::*;

trait A {
    type AssocType;
}

struct Foo;
impl A for Foo {
    type AssocType = i32;
}

#[trusted]
#[refine_spec(where T: A<AssocType = u32>, [
    ensures(result > 0)
])]
fn foo<T>(x: T) -> i32 {
    42
}

fn associated_type_mismatch() {
    let f = Foo;
    let res = foo(f);
    assert!(res > 0); //~ ERROR: might not hold
}

trait AWithParam<T> {
    type AssocType;
}

struct FooWithParam;
impl AWithParam<i32> for FooWithParam {
    type AssocType = i32;
}

#[trusted]
#[refine_spec(where T: AWithParam<i32, AssocType = u32>, [
    ensures(result > 0)
])]
fn foo_with_param<T>(x: T) -> i32 {
    42
}

fn associated_type_mismatch_with_param() {
    let f = FooWithParam;
    let res = foo_with_param(f);
    assert!(res > 0); //~ ERROR: might not hold
}

trait NestedA {}
trait NestedB<X> {}
trait NestedC<Y> {}

impl NestedA for i32 {}
// impl NestedB<i32> for i32 {}
impl NestedB<u32> for i32 {}

#[trusted]
#[refine_spec(where T: NestedA + NestedB<i32> + NestedB<u32>, [
    ensures(result > 0)
])]
fn foo_nested<T>(_x: T) -> i32 {
    42
}

fn main() {
    let result = foo_nested(1);
    assert!(result > 0); //~ ERROR: might not hold
}
