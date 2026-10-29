use prusti_contracts::*;

#[pure]
fn foo<T>(x: &T) -> bool { true }

#[ensures(foo(&x))]
fn bar<T>(x: T) {
    drop(x);
}

fn drop<T>(x: T) {}

#[ensures(foo(&x))]
fn baz<T>(x: T, v: i32) {
    if v > 0 { baz(x, v-1) }
}

struct T {
    x: u32,
}

#[pure]
fn get_x(t: &T) -> u32 {
    t.x
}

#[ensures(old(get_x(t)) == get_x(t))]
fn test(t: &mut T) {}

#[ensures(forall(|y: u32| forall(|z: u32| false ==> (y + z > old(*x)))))]
fn test_nested_quantifiers(x: &mut u32) {}
