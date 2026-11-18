// Regression test for prusti/prusti#749: derived PartialEq on enums with multi-field variants.
#[derive(PartialEq)]
pub enum Binop {
    Add(u32, u32),
    Sub(u32, u32),
}

#[prusti_contracts::ensures(result)] //~ ERROR: postcondition might not hold
pub fn different_fields_differ() -> bool {
    Binop::Add(1, 2) == Binop::Add(1, 3)
}

#[prusti_contracts::ensures(!result)] //~ ERROR: postcondition might not hold
pub fn same_value_equal() -> bool {
    Binop::Sub(4, 5) == Binop::Sub(4, 5)
}
