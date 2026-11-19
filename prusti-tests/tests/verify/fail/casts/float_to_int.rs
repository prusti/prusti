use prusti_contracts::*;

#[requires(a as f32 as i32 == 0)]
fn cast_in_spec(a: i64) {}

fn call_cast_in_spec() {
    cast_in_spec(1); //~ ERROR: precondition might not hold
}
