use prusti_contracts::*;

fn main() {
    let n = [42, 24];
    let [x, y] = n;
    assert!(x == 42);
    assert!(y == 24);
}

#[trusted]
#[ensures(forall(|k: usize| k < 4 ==> result[k] == 7, triggers=[(result[k],)]))]
fn sevens() -> [u32; 4] {
    [7; 4]
}

// The quantifier is instantiated at the constant index of the pattern.
fn pattern_after_call() {
    let [_, b, _, _] = sevens();
    assert!(b == 7);
}

#[pure]
fn second(a: [u32; 4]) -> u32 {
    let [_, b, _, _] = a;
    b
}

#[requires(forall(|k: usize| k < 4 ==> a[k] == 7, triggers=[(a[k],)]))]
fn pure_pattern(a: [u32; 4]) {
    assert!(second(a) == 7);
}
