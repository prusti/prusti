//@ compile-flags: -Passert_timeout=45000
// `Cell` with different content types (Copy and non-Copy structs, `Option`)
// and the `Cell` trait implementations (`Clone`, `Default`, `From`). The
// cases Prusti does not verify yet are in `cell_types_failing.rs`.

use prusti_contracts::cell::Cell;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Point {
    x: i32,
    y: i32,
}

fn copy_struct_content() {
    let c = Cell::new(Point { x: 1, y: 2 });
    let p = c.get();
    c.set(Point { x: p.y, y: p.x });
    assert!(c.get().x == 2);
    assert!(c.get().y == 1);
}

struct Pair {
    a: i32,
    b: i32,
}

fn non_copy_content() {
    let c = Cell::new(Pair { a: 1, b: 2 });
    let old = c.replace(Pair { a: 3, b: 4 });
    assert!(old.a == 1 && old.b == 2);
    let cur = c.into_inner();
    assert!(cur.a == 3 && cur.b == 4);
}

fn option_content() {
    let c = Cell::new(Some(3));
    let v = c.take();
    assert!(v == Some(3));
    c.set(Some(4));
    assert!(c.get() == Some(4));
}

fn clone_is_independent() {
    let a = Cell::new(1);
    let b = a.clone();
    a.set(5);
    assert!(a.get() == 5);
    assert!(b.get() == 1);
}

fn default_then_set() {
    let c: Cell<i32> = Cell::default();
    c.set(2);
    assert!(c.get() == 2);
}

fn from_value() {
    let c = Cell::from(8);
    assert!(c.get() == 8);
}

fn main() {
    copy_struct_content();
    non_copy_content();
    option_content();
    clone_is_independent();
    default_then_set();
    from_value();
}
