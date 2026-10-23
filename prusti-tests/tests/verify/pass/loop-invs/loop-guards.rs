use prusti_contracts::*;

fn test_composed_guard() {
    let mut i = 0;
    while i < 10 || true {
        i += 1;
    }
}

fn test_return_in_guard() {
    let mut i = 0;

    while {
        body_invariant!(i == 0); // TODO: loop framing should guarantee this
        if i < 10 {
            return;
        }
        i < 55
    } {
        i += 1;
        assert!(false); // Unreachable
    }

    assert!(i == 55);
}

fn test_break_continue() {
    let mut i = 0;

    while i < 10 {
        body_invariant!(i < 10);
        'inner: while {
            i += 1;
            i -= 1;
            i < 10
        } {
            body_invariant!(i < 10);
            i += 1;
            if i == 234 {
                break;
            }
            if i == 345 {
                continue;
            }
        }
        assert!(i == 10);
    }

    assert!(i == 10);
}
