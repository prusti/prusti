use prusti_contracts::*;

// A call of a closure goes to the stub of an `Fn*` trait method, which does not
// take the targets of mutable references that the closure captures or takes
// as parameters (they are inside the type parameters `Self` and `Args`): the
// caller keeps them, unchanged. So a closure whose body needs them is
// rejected. Its body is still verified.

fn mut_capture() {
    let mut y = 0;
    let _set = || { //~ERROR: the closure needs the targets of mutable references
        y = 1;
        prusti_assert!(y == 1);
    };
}

fn body_does_not_know_captured_value() {
    let mut y = 0;
    let _set = || { //~ERROR: the closure needs the targets of mutable references
        prusti_assert!(y == 0); //~ERROR: the asserted expression might not hold
        y = 1;
    };
}

fn mixed_captures() {
    let n = 2;
    let mut t = String::new();
    let u = String::new();
    let _f = || { //~ERROR: the closure needs the targets of mutable references
        t.push_str("t");
        std::mem::drop(u);
        let _ = n;
    };
}

// An `FnMut` closure changes its own state through `&mut self`, which a call
// gives it.
fn own_state() {
    let mut z = 0;
    let _count = move || {
        z += 1;
        z
    };
}

fn mut_param() {
    let _set = |x: &mut i32| *x = 1; //~ERROR: the closure needs the targets of mutable references
}

fn shared_capture() {
    let n = 2;
    let _f = |x: i32| x + n;
}

fn by_value() {
    let s = String::new();
    let _f = move || std::mem::drop(s);
}

// The caller regains a captured place once the closure is dead.
fn after_definition() {
    let mut y = 0;
    {
        let _set = || y = 1; //~ERROR: the closure needs the targets of mutable references
    }
    y = 2;
    assert!(y == 2);
}
