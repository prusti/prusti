enum Either {
    Left(i32),
    Right(i32),
}

fn to_right(x: Either, y: Either) -> Either {
    match x {
        Either::Left(val) |
        Either::Right(val) => {
            Either::Right(val)
        }
    }
}

enum Enum {
    One(i32),
    Two(i32),
    Three(i32),
}

fn random() -> bool {
    true
}

fn test() -> Enum {
    if random() {
        Enum::One(111)
    } else {
        Enum::Two(222)
    }
    // At the join point we don't have `Enum::Three(i32)`
}

enum IntOption {
    Some(i32),
    None
}

fn foo(x: IntOption) -> i32 {
    let y = IntOption::Some(123);
    match x {
        IntOption::Some(y) => y,
        IntOption::None => 456
    }
}

#[derive(Copy, Clone)]
enum MyOption {
    Some((i32, i64)),
    None
}

fn copy_nested(x: ((i32, MyOption), bool)) -> ((i32, MyOption), bool) {
    let y = x;
    let z = x;
    y
}

enum Expr {
    Sum(i32, i32)
}

fn eval(expr: Expr) -> i32 {
    match expr {
        Expr::Sum(left, right) => left + right
    }
}
