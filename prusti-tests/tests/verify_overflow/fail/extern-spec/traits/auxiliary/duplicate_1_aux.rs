use prusti_contracts::*;

pub trait MyTrait {
    #[requires(true)]
    fn foo(&mut self) -> i32;
}
