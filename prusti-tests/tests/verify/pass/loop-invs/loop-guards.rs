use prusti_contracts::*;

fn test_composed_guard() {
    let mut i = 0;
    while i < 10 || true {
        i += 1;
    }
}

fn test_loop_in_guard() {
    let mut i = 0;

    while {
        body_invariant!(i % 10 == 0 && i < 60);
        let old_i = i;
        while i < old_i + 10 {
            body_invariant!(i < old_i + 10);
            i += 1;
        }
        i < 55
    } {
        continue
    }

    assert!(i == 60);
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

fn test_complex_guard() {
    let mut i = 0;

    'outer: loop {
        body_invariant!(i == 0);
        while {
            let old_i = i;
            while i < old_i + 10 {
                body_invariant!(old_i < 55); // TODO: loop framing should guarantee this
                body_invariant!(i < old_i + 10);
                if i == 123123 {
                    break;
                }
                if i == 234234 {
                    continue;
                }
                i += 1;
            }
            i < 55
        } {
            body_invariant!(i % 10 == 0 && i < 55);
            i += 1;
            if i == 456456 {
                break;
            }
            if i == 567567 {
                continue;
            }
            i -= 1;
        }

        if i > 55 {
            break 'outer;
        } else {
            continue;
        }
    }

    assert!(i == 60);
}
