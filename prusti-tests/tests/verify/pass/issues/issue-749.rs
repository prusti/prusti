// Regression test for prusti/prusti#749: derived PartialEq on enums with multi-field variants.
#[derive(PartialEq)]
pub enum Binop {
    Add(u32, u32),
    Sub(u32, u32),
}

#[derive(PartialEq)]
pub enum Binop2 {
    Add { left: u32, right: u32 },
    Sub { left: u32, right: u32 },
}

#[prusti_contracts::ensures(result)]
pub fn same_variant_equal() -> bool {
    Binop::Add(1, 2) == Binop::Add(1, 2)
}

#[prusti_contracts::ensures(!result)]
pub fn different_variants_differ() -> bool {
    Binop2::Add { left: 1, right: 2 } == Binop2::Sub { left: 1, right: 2 }
}
