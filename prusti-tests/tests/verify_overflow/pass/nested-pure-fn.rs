//@ known-failure
use prusti_contracts::*;

struct Struct {
    field: u32,
}

#[pure]
fn inner(m: &Struct) -> &u32 {
    &m.field
}

#[pure]
fn outer(field: &u32) -> bool {
    true
}

#[pure]
fn pred(m: &Struct) -> bool {
    outer(inner(&m))
}
