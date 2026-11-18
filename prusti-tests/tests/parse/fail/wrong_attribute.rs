use prusti_contracts::*;

#[require(x!=0)] //~ERROR: cannot find attribute
#[ensures(result!=0)]
fn wrong(x: i32) -> i32 {
    x
}

#[pure]
#[requires] //~ERROR: the `#[requires]` attribute expects an argument in parentheses
fn requires_without_argument() -> bool {
    true
}

#[pure]
#[ensures = "true"] //~ERROR: the `#[ensures]` attribute expects an argument in parentheses
fn ensures_with_value() -> bool {
    true
}

#[pure]
#[trusted(x)] //~ERROR: the `#[trusted]` attribute does not take parameters
fn trusted_with_parameter() -> bool {
    true
}

#[pure]
#[invariant(true)]
fn invariant_on_function() -> bool { //~ERROR: expected one of: `struct`, `enum`, `union`
    true
}

#[trusted(x)] //~ERROR: the `#[trusted]` attribute does not take parameters
struct TrustedWithParameter;

#[print_counterexample]
#[model(x)] //~ERROR: the `#[model]` attribute does not take parameters
struct ModelWithParameter {
    a: i32,
}

trait Trait {
    fn f(&self) -> bool;
}

struct S;

#[refine_trait_spec]
impl Trait for S {
    #[predicate] //~ERROR: cannot find attribute `predicate`
    fn f(&self) -> bool {
        true
    }
}
