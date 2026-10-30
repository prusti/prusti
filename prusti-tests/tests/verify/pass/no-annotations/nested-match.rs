enum Num {
    Integer(i32),
    Rational(i32, i32),
}

enum Expr {
    Sum(Num, Num),
    Diff(Num, Num),
    Constant(Num)
}

fn check(expr: Expr) -> bool {
    let result = match expr {
        Expr::Sum(left, right) |
        Expr::Diff(left, right) => {
            match left {
                Num::Integer(_) => match right {
                    Num::Integer(_) => true,
                    _ => false,
                },
                Num::Rational(_, left_den) => match right {
                    Num::Rational(_, right_den) => left_den != 0 && right_den != 0,
                    _ => false,
                }
            }
        }
        Expr::Constant(Num::Integer(_)) => true,
        Expr::Constant(Num::Rational(_, den)) => den != 0
    };

    result
}

enum Either {
    Left,
    Right,
}

enum OptionEither {
    Some(Either),
    None,
}

fn none_or_left(x: OptionEither) -> bool {
    match x {
        OptionEither::None => true,
        OptionEither::Some(Either::Left) => true,
        OptionEither::Some(Either::Right) => false,
    }
    // Here we had an issue with the first implementation of enumerations for the fold/unfold:
    // 1. The `OptionEither::None` branch has a `Either(x.enum_0_0)` predicate.
    // 2. The `OptionEither::Some(_)` branches has a `int(x.enum_0_0.discriminant)` predicate.
    // 3. At join point, the `OptionEither::None` branch tries to unfold `Either(x.enum_1_0)`
    //    to reach the predicate `int(x.enum_1_0.discriminant)` on which the branches seem to agree.
}

// This struct can not be copied or cloned
struct T;

enum EitherT {
    Left(T),
    Right,
}

enum OptionEitherT {
    Some(EitherT),
}

fn test(x: OptionEitherT) {
    // In this particular situation the match does not move out values from the enumeration
    let y = match x {
        OptionEitherT::Some(EitherT::Left(_)) => true,
        OptionEitherT::Some(EitherT::Right) => false,
    };
    let z = match x {
        OptionEitherT::Some(EitherT::Left(_)) => true,
        OptionEitherT::Some(EitherT::Right) => false,
    };
    assert!(y == z);
}
