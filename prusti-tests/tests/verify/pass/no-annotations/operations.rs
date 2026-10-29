fn main() {
    let a = 10;
    let b = (a + 2) - 6;  // 6
    let c = (b - 1) * 2;  // 10
    let d = -c;  // -10
    let x = (0 < d) || (d > 10);  // true
    let y = (10 >= d) && (d <= 0);  // false
    let z = (a == b) || (x != y); // true
    assert!(d == -10 && z);

    assert!(9 / 2 == 4);
}

// Compound assignments on primitives lower to a same-place form, e.g.
// `x /= y` to `x = Div(copy x, move y)`: the destination is also an operand,
// so its snapshot must be read before the assignment exhales its predicate.

fn compound_assign(x: i32, y: i32) -> i32 {
    let mut x = x;
    if y != 0 {
        x /= y;
        x %= 7;
    }
    x
}

fn compound_assign_field(t: (i32, i32)) -> (i32, i32) {
    let mut t = t;
    if t.1 != 0 {
        t.0 /= t.1;
    }
    t
}

fn test_and() {
    assert!(true  & true  == true );
    assert!(true  & false == false);
    assert!(false & true  == false);
    assert!(false & false == false);
}

fn test_or() {
    assert!(true  | true  == true );
    assert!(true  | false == true );
    assert!(false | true  == true );
    assert!(false | false == false);
}

fn test_xor() {
    assert!(true  ^ true  == false);
    assert!(true  ^ false == true );
    assert!(false ^ true  == true );
    assert!(false ^ false == false);
}

fn foo(a: u8, b: i8, c: usize, d: isize) {
    assert!(a as char as u8 as u16 as u32 as u64 as u128 == a as u128);
    assert!(b as i8 as i16 as i32 as i64 as i128 == b as i128);
    assert!(c as usize == c);
    assert!(d as isize == d);
}

fn test(x: i32) {
    let is_pos = x.is_positive();
    let is_neg = x.is_negative();
}
