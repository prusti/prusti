use prusti_contracts::*;

#[pure]
pub fn get(a: &usize) -> usize {
    *a
}
fn foo(a: &usize) {
    let v = get(a);
}
