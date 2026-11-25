use prusti_contracts::*;

#[extern_spec(crate)] //~ERROR: Extern specs cannot be attached to this item
struct S;
