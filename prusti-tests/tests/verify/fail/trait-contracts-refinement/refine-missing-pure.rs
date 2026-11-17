//! A `#[refine_trait_spec]` implementation takes over the specification of the
//! trait method it refines. If the trait method is `#[pure]` but the refining
//! implementation omits `#[pure]`, it would refine a pure method into an impure
//! one, which is rejected. So is an implementation in `#[pure(memory)]` mode,
//! whose result may depend on addresses the trait method's result must not.

use prusti_contracts::*;

trait Trait {
    #[pure]
    fn foo(&self) -> i32;
}

struct Struct;

#[refine_trait_spec]
impl Trait for Struct {
    // Missing `#[pure]` while refining a pure trait method.
    #[ensures(result == 5)]
    fn foo(&self) -> i32 { //~ ERROR: implements a `#[pure]` trait method and so must itself be `#[pure]`
        5
    }
}

trait MemoryTrait {
    #[pure(memory)]
    fn get(&self) -> &i32;
}

#[refine_trait_spec]
impl MemoryTrait for Struct {
    // Missing `#[pure(memory)]` (or `#[pure]`) while refining a pure trait method.
    #[ensures(true)]
    fn get(&self) -> &i32 { //~ ERROR: implements a `#[pure(memory)]` trait method and so must itself be `#[pure(memory)]` or `#[pure]`
        &5
    }
}

struct Memory;

#[refine_trait_spec]
impl Trait for Memory {
    #[pure(memory)]
    fn foo(&self) -> i32 { //~ ERROR: implements a `#[pure]` trait method, so its result must not depend on memory addresses
        5
    }
}
