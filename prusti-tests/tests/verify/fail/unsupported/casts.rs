use prusti_contracts::*;

// Turning a function item into a function pointer and float-to-float casts are
// not supported.

fn id(x: u32) -> u32 {
    x
}

fn reify() {
    let _f: fn(u32) -> u32 = id; //~ERROR: the pointer coercion `ReifyFnPointer(Safe)`
}

#[pure]
fn reify_pure() -> bool {
    let _f: fn(u32) -> u32 = id; //~ERROR: the pointer coercion `ReifyFnPointer(Safe)`
    true
}

fn widen(x: f32) -> f64 {
    x as f64 //~ERROR: the cast `FloatToFloat`
}

#[pure]
fn widen_pure(x: f32) -> f64 {
    x as f64 //~ERROR: the cast `FloatToFloat`
}
