#![feature(never_type)]

fn main() {
    panic!();  //~ERROR: might not hold
}

fn calls_unimplemented(x: i32) -> i32 {
    unimplemented!(); //~ERROR: might not hold
}

fn calls_unreachable(x: i32) -> i32 {
    unreachable!(); //~ERROR: might not hold
}

fn diverging() -> ! {
    panic!();  //~ERROR: might not hold
}

struct IntBox {
    val: i32
}

enum Expr {
    Sum(IntBox, IntBox),
    Constant(IntBox)
}

fn compute(expr: Expr) -> i32 {
    let simplified = match expr {
        Expr::Sum(a, b) => Expr::Sum(b, a),
        x => x
    };

    let value = match simplified {
        Expr::Sum(_, _) => {
            unreachable!()  //~ERROR: might not hold
        },
        Expr::Constant(IntBox { val }) => val
    };

    value
}
