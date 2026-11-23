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
fn invariant_on_function() -> bool { //~ERROR: the `#[invariant]` attribute can only be applied to structs, enums and unions, and this item could not be parsed as one: expected one of: `struct`, `enum`, `union`
    true
}

#[trusted(x)] //~ERROR: the `#[trusted]` attribute does not take parameters
#[invariant(true)]
struct TrustedWithParameter;

#[trusted]
#[print_counterexample("a = {}", a)] //~ERROR: this attribute has no effect on a `#[trusted]` type
struct TrustedWithCounterexample {
    a: i32,
}

#[invariant(self.a > 0)] //~ERROR: this attribute has no effect on a `#[trusted]` type
#[trusted]
struct TrustedWithInvariant {
    a: i32,
}

#[trusted]
#[trusted] //~ERROR: duplicate `#[trusted]` attribute
struct TrustedTwice;

#[print_counterexample("a = {}", a)]
fn counterexample_on_function() {} //~ERROR: the `#[print_counterexample]` attribute can only be applied to structs and enums, and this item could not be parsed as one

pub struct Unspecified;

#[trusted]
impl Unspecified {} //~ERROR: the `#[trusted]` attribute can only be applied to functions, structs, enums and unions, and this item could not be parsed as one: expected `fn`

#[print_counterexample("a")] //~ERROR: the `#[print_counterexample]` attribute can only be applied to structs and enums
union CounterexampleUnion {
    a: i32,
}

#[ensures(result > 0)]
#[unsafe(no_mangle)] //~ERROR: the `#[ensures]` attribute can only be applied to functions, and this item could not be parsed as one: expected path
pub extern "C" fn unparsable() -> i32 {
    1
}

// A `pub` function that syn cannot parse is reported where syn fails (at
// `use`), not at `pub`.
#[ensures(true)]
pub fn unparsable_pub() -> impl Sized + use<> { //~ERROR: the `#[ensures]` attribute can only be applied to functions, and this item could not be parsed as one
    0
}

#[trusted]
#[pure]
struct TrustedPure; //~ERROR: the `#[pure]` attribute can only be applied to functions, and this item could not be parsed as one: expected `fn`

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
#[model] //~ERROR: duplicate `#[model]` attribute
struct Modelled {
    a: i32,
}

#[model]
#[pure] //~ERROR: the `#[pure]` attribute can only be applied to functions
struct Modelled {
    a: i32,
}

#[model]
#[trusted] //~ERROR: this attribute cannot be combined with `#[model]`
struct Modelled {
    a: i32,
}

pub enum ModelledEnum {
    A,
}

#[model] //~ERROR: the `#[model]` attribute can only be applied to structs
#[invariant(true)]
enum ModelledEnum {
    A,
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
