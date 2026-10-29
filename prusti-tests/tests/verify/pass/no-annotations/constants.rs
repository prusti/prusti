const C: (i32, i32) = (0, 0);

fn tuple_field() {
    let _ = C.0;
}

fn foo(_x: i32) {}

fn argument_in_loop(x: i32) {
    let mut i = 100;
    while i > 0 {
        if i == 10 {
            foo(123);
        }
        i -= 1;
    }
}
