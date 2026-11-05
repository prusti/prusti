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
    unsafe { u.a } //~ERROR: cannot encode function body `read_pure`: the fields of `U` cannot be used
    //~^ ERROR: cannot encode method body `read_pure`: the fields of `U` cannot be used
}

fn read(u: U) -> u32 {
    unsafe { u.a } //~ERROR: the fields of `U` cannot be used
}

fn write(mut u: U) -> U {
    u.b = 5; //~ERROR: the fields of `U` cannot be used
    u
}

fn construct() -> U {
    U { a: 1 } //~ERROR: unsupported rvalue U
}
