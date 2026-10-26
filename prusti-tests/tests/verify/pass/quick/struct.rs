use prusti_contracts::*;

struct Foo {
    bar: i32,
    r#type: i32
}

#[requires(foo.bar == 1)]
fn check_bar_fn(foo: &Foo) {
}

#[requires(foo.r#type == 1)]
fn check_rtype_fn(foo: &Foo) {
}

#[pure]
fn rtype_same(foo1: &Foo, foo2: &Foo) -> bool {
    foo1.r#type == foo2.r#type
}

// Named like the built-in array type; its constructor's fn item type is a
// type of its own.
struct Array(u8);

fn constructor_as_value(_a: [u8; 1]) -> Array {
    let make = Array;
    make(0)
}

fn main(){
}
