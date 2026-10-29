// Two type-conditional spec refinements with different bounds; neither applies to the call in
// main.
use prusti_contracts::*;

trait A {}
trait B {}

#[trusted]
#[refine_spec(where T: A, [
    requires(true),
    ensures(true),
])]
#[refine_spec(where T: B, [
    requires(true),
    ensures(true),
])]
fn foo<T>(_x: T) {
}

fn main() {
    foo::<i32>(42);
}
