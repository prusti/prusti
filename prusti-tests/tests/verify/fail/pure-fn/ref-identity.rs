use prusti_contracts::*;

/// Calls of a pure function dummy the addresses of its shared-reference
/// arguments, so a pure function whose result depends on them is rejected.
#[pure]
#[ensures(result as *const i32 == x as *const i32)]
fn id(x: &i32) -> &i32 { //~ERROR: might not match its definition with the addresses of its shared-reference arguments ignored
    x
}
