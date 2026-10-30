use prusti_contracts::*;

struct Point {
    x: i32,
    y: i32,
}

#[ensures(*result == pt.x)]
fn get_x<'a, 'b: 'a>(pt: &'b Point) -> &'a i32 {
    &pt.x
}

fn main() {
    let pt = Point { x: 42, y: 72 };
    let x = get_x(&pt);
    assert!(*x == 42);
}

fn reborrow<U>(x: &mut U) -> &mut U {
    x
}

fn caller<T>(mut v: T) {
    let _ = reborrow(&mut v);
}
