use prusti_contracts::*;

// The body of a closure gets the permissions of what it captures, including
// the targets of mutable references. The closures are not called: calls of
// closures capturing by mutable reference are not supported yet (see
// `mut-capture.rs`).

fn mut_capture() {
    let mut y = 0;
    let _set = || {
        y = 1;
        prusti_assert!(y == 1);
    };
}

fn mut_capture_string() {
    let mut t = String::new();
    let _push = || t.push_str("t");
}

fn mixed_captures() {
    let n = 2;
    let mut y = 0;
    let mut t = String::new();
    let u = String::new();
    let _f = || {
        y += n;
        t.push_str("t");
        std::mem::drop(u);
    };
}

fn nested() {
    let mut y = 0;
    let _outer = || {
        let _inner = || y += 1;
    };
}

// The caller regains a captured place once the closure is dead.
fn after_definition() {
    let mut y = 0;
    {
        let _set = || y = 1;
    }
    y = 2;
    assert!(y == 2);
}
