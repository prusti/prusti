//@ compile-flags: -Passert_timeout=45000
// Review round 9: `#[pure]` functions see deep values, also generic ones
// instantiated with interior-mutable types (derived and library impls).
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

#[derive(PartialEq)]
struct W<T>(T);

fn derived_generic() {
    let x = W(Cell::new(1)) == W(Cell::new(2));
    let y = W(Cell::new(3)) == W(Cell::new(3));
    assert!(x == y); //~ ERROR
}

#[pure]
fn same<T: PartialEq>(a: &T, b: &T) -> bool {
    a == b
}

fn generic_pure_fn() {
    let a = Cell::new(1);
    let b = Cell::new(2);
    let c = Cell::new(3);
    let d = Cell::new(3);
    if same(&c, &d) {
        assert!(same(&a, &b)); //~ ERROR
    }
}

fn option_eq() {
    let x = Some(Cell::new(1)) == Some(Cell::new(2));
    let y = Some(Cell::new(3)) == Some(Cell::new(3));
    assert!(x == y); //~ ERROR
}

fn ctrl() { assert!(false); } //~ ERROR
