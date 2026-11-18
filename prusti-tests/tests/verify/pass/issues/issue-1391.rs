use prusti_contracts::*;

// Regression test for prusti/prusti#1391: snapshot (in)equality of enum variants.
enum MyEnum {
    A,
    B,
}

fn variants_differ() {
    prusti_assert!(MyEnum::A !== MyEnum::B);
    prusti_assert!(MyEnum::A === MyEnum::A);
}
