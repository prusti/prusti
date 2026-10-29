use prusti_contracts::*;

#[trusted]
fn random() -> u32 {
    unimplemented!()
}

// A loan created in the guard, before the loop invariant, that expires after it.
fn test_borrow_in_guard() {
    let mut x: &mut Box<i32>;
    let mut y = Box::new(123);
    while {
        x = &mut y;
        true
    } {
        drop(x);
    }
}

fn test_initialize_in_guard() {
    let mut x: Box<u32>;

    'myloop: while {
        x = Box::new(random());
        if *x == 0 {
            break 'myloop;
        }
        *x < 55
    } {
        body_invariant!(*x < 55);
        assert!(*x != 100);
    }

    assert!(*x == 0 || *x >= 55);
}

fn test_tmp_in_guard() {
    let mut tmp = Box::new(Box::new(random()));
    let tmp_ref = &tmp;
    let tmp_ref_mut = &mut tmp;

    while {
        let guard = random() < 55;

        let mut tmp = Box::new(Box::new(random()));
        let tmp_ref = &tmp;
        let tmp_ref_mut = &mut tmp;

        guard
    } {
        let mut tmp = Box::new(Box::new(random()));
        let tmp_ref = &tmp;
        let tmp_ref_mut = &mut tmp;
    }

    let mut tmp = Box::new(Box::new(random()));
    let tmp_ref = &tmp;
    let tmp_ref_mut = &mut tmp;

    assert!(true);
}
