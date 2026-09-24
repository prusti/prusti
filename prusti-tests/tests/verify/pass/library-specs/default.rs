use prusti_contracts::*;

fn integers() {
    let a: i32 = Default::default();
    let b: u64 = Default::default();
    let c: usize = usize::default();
    assert!(a == 0 && b == 0 && c == 0);
}

fn primitives() {
    let b: bool = Default::default();
    let c: char = Default::default();
    let u: () = Default::default();
    let f: f64 = Default::default();
    assert!(!b);
    assert!(c == '\0');
    assert!(u == ());
    assert!(f == 0.0);
}

fn option() {
    let o: Option<i32> = Default::default();
    assert!(matches!(o, None));
}

fn tuples() {
    let t: (i32, bool) = Default::default();
    assert!(t.0 == 0);
    assert!(!t.1);
    let u: (u8, (), char) = Default::default();
    assert!(u.0 == 0);
    assert!(u.2 == '\0');
    let n: ((i8,), Option<u8>, (bool, char)) = Default::default();
    assert!(n.0 .0 == 0);
    assert!(matches!(n.1, None));
    assert!(!n.2 .0);
}

fn longest_tuple() {
    let t: (u8, u16, u32, u64, i8, i16, i32, i64, bool, char, (), usize) = Default::default();
    assert!(t.0 == 0 && t.3 == 0 && t.7 == 0);
    assert!(!t.8 && t.9 == '\0');
    assert!(t.11 == 0);
}
