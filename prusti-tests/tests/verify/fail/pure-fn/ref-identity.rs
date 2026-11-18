use prusti_contracts::*;

/// Calls of a pure function dummy the addresses of its shared-reference
/// arguments, so a pure function whose result depends on them is rejected.
#[pure]
#[ensures(result as *const i32 == x as *const i32)]
fn id(x: &i32) -> &i32 { //~ERROR: might depend on memory addresses
    x
}

/// A `#[pure(memory)]` function may depend on the addresses, so equal
/// contents do not make two calls equal.
#[pure(memory)]
fn id_memory(x: &i32) -> &i32 {
    x
}

fn memory_equal_contents(a: &i32, b: &i32) {
    if *a == *b {
        prusti_assert!(id_memory(a) === id_memory(b)); //~ERROR: the asserted expression might not hold
    }
}

/// Pure functions cannot observe the addresses of their own local variables,
/// even in memory mode.
#[pure(memory)]
fn local_addresses(x: u32) -> bool { //~ERROR: might depend on the addresses of its local variables
    let a = x;
    let b = x;
    &a as *const u32 == &b as *const u32
}

fn memory_wrong_referent(a: &i32) {
    prusti_assert!(*id_memory(a) != *a); //~ERROR: the asserted expression might not hold
}
