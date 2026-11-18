// Regression test for prusti/prusti#1391: snapshot (in)equality of enum variants.
use prusti_contracts::*;

enum MyEnum {
    A,
    B,
}

fn variants_equal() {
    prusti_assert!(MyEnum::A === MyEnum::B); //~ ERROR: might not hold
}

fn variant_unequal_to_itself() {
    prusti_assert!(MyEnum::A !== MyEnum::A); //~ ERROR: might not hold
}
