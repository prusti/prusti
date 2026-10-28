use prusti_contracts::*;

// The refinement check compares the postconditions over the states before and
// after the call, so `old` in them is not the current value.
trait T {
    #[ensures(*x == old(*x) + 2)]
    fn bump(&self, x: &mut i32);
}

struct S;

#[refine_trait_spec]
impl T for S {
    #[ensures(*x == old(*x) + 1)] //~ ERROR: the implementation's postcondition may be weaker than the trait method's
    fn bump(&self, x: &mut i32) {
        *x += 1;
    }
}
