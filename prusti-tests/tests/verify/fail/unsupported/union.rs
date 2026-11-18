use prusti_contracts::*;

// A union is encoded as an opaque value, so its fields cannot be read or
// written, and its values cannot be built from fields.

#[derive(Clone, Copy)]
union U {
    a: u32,
    b: u32,
}

#[pure]
fn read_pure(u: U) -> u32 {
    unsafe { u.a } //~ERROR: cannot encode function body `read_pure`: Prusti does not look inside `U`
    //~^ HELP: move this code into a `#[trusted]` function
}

fn read(u: U) -> u32 {
    unsafe { u.a } //~ERROR: Prusti does not look inside `U`
    //~^ HELP: move this code into a `#[trusted]` function
}

fn write(mut u: U) -> U {
    u.b = 5; //~ERROR: Prusti does not look inside `U`
    //~^ HELP: move this code into a `#[trusted]` function
    u
}

fn construct() -> U {
    U { a: 1 } //~ERROR: Prusti does not look inside `U`
    //~^ HELP: move this code into a `#[trusted]` function
}
