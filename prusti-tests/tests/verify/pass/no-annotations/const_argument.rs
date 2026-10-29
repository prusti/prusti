use prusti_contracts::*;

fn foo(_x: i32) {}

fn test(x: i32) {
    let mut i = 100;
    while i > 0 {
        if i == 10 {
            foo(123);
        }
        i -= 1;
    }
}
