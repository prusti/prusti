use prusti_contracts::*;

pub fn simple_loop() {
    let mut x = 0;
    while x < 100 {
        body_invariant!(x == 42); //~ERROR: loop invariant might not hold
        x += 1;
    }
}

#[trusted]
fn random_bool() -> bool {
    unimplemented!()
}

fn test() {
    let mut y = None;
    let mut z = None;

    loop {
        let x = Box::new(5);
        if random_bool() {
            y = Some(x);
        } else {
            z = Some(x);
        }
        assert!(false); //~ERROR: might not hold
    }
}

#[trusted]
fn random() -> u32 {
    unimplemented!()
}

fn test1() {
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
    assert!(false); //~ ERROR: might not hold
}

fn test2() {
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
        assert!(false); //~ ERROR: might not hold
    }

    let mut tmp = Box::new(Box::new(random()));
    let tmp_ref = &tmp;
    let tmp_ref_mut = &mut tmp;
}
