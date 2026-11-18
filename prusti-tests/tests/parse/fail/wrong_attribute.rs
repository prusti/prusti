use prusti_contracts::*;

#[require(x!=0)] //~ERROR: cannot find attribute
#[ensures(result!=0)]
fn wrong(x: i32) -> i32 {
    x
}

#[pure]
#[requires] //~ERROR: this attribute expects an argument
fn requires_without_argument() -> bool {
    true
}

#[ensures] //~ERROR: this attribute expects an argument
#[pure]
fn ensures_without_argument() -> bool {
    true
}

#[pure]
#[ensures = "true"] //~ERROR: `#[ensures = ...]` is not supported
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

#[trusted]
#[print_counterexample("a = {}", a)]
struct TrustedWithCounterexample { //~ERROR: `trusted!` is incompatible with other Prusti attributes
    a: i32,
}

// The type that the `#[model]` declarations below describe.
pub struct Modelled {
    a: i32,
}

#[model(x)] //~ERROR: the `#[model]` attribute does not take parameters
struct Modelled {
    a: i32,
}

#[model]
#[invariant(self.a > 0)] //~ERROR: this attribute cannot be combined with `#[model]`
struct Modelled {
    a: i32,
}

#[invariant(self.a > 0)] //~ERROR: this attribute cannot be combined with `#[model]`
#[model]
struct Modelled {
    a: i32,
}

#[model]
#[model] //~ERROR: this attribute cannot be combined with `#[model]`
struct Modelled {
    a: i32,
}

pub fn closure_with_parameter() {
    let _ = closure!(
        #[pure(x)] //~ERROR: the `#[pure]` attribute does not take parameters
        |x: i32| -> i32 { x }
    );
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
