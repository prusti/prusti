use prusti_contracts::*;

#[extern_spec(crate)]
unsafe extern "C" { //~ERROR: Extern specs cannot be attached to this item
    fn abs(a: i32) -> i32;
}
