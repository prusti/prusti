//@ known-failure
//@ compile-flags: -Passert_timeout=45000
// The `Cell` cases of `cell_types.rs` Prusti does not verify yet:
// - interior-mutable values inside a `Cell`, and slices of cells (not
//   supported yet);
// - `==` on the contents of a generic or primitive type, which goes through
//   the `PartialEq` trait (whose calls have no semantics for foreign impls,
//   and which is not reflexive in general).

use prusti_contracts::cell::Cell;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Point {
    x: i32,
    y: i32,
}

fn nested_cell() {
    let outer = Cell::new(Cell::new(1));
    let inner = outer.replace(Cell::new(2));
    assert!(inner.get() == 1);
    assert!(outer.into_inner().get() == 2);
}

fn generic_set_then_get<T: Copy + PartialEq>(c: &Cell<T>, v: T) {
    c.set(v);
    assert!(c.get() == v);
}

fn generic_callers() {
    let a = Cell::new(0u8);
    generic_set_then_get(&a, 5);
    let b = Cell::new(false);
    generic_set_then_get(&b, true);
    let p = Cell::new(Point { x: 0, y: 0 });
    generic_set_then_get(&p, Point { x: 1, y: 1 });
}

fn slice_of_cells() {
    let mut data = [1, 2, 3];
    let all: &Cell<[i32]> = Cell::from_mut(&mut data[..]);
    let cells: &[Cell<i32>] = all.as_slice_of_cells();
    cells[0].set(10);
    cells[2].set(30);
    assert!(cells[0].get() == 10);
    assert!(cells[1].get() == 2);
    assert!(cells[2].get() == 30);
    assert!(data[0] == 10 && data[1] == 2 && data[2] == 30);
}

fn comparisons_use_contents() {
    let a = Cell::new(1);
    let b = Cell::new(1);
    assert!(a == b);
    b.set(2);
    assert!(a != b);
    assert!(a < b);
    assert!(b > a);
}

fn main() {
    nested_cell();
    generic_callers();
    slice_of_cells();
    comparisons_use_contents();
}
