use prusti_contracts::*;

pub struct S {
    pub v: i32,
}

fn consume(_s: S) {}

fn consume_b(_s: S) -> bool {
    true
}

// The guard temporary `t` is assigned before the loop head and again on the
// exiting iteration. Its first value must be dropped when its storage dies:
// if it were leaked, the second assignment would add another full predicate
// for the same place, and two full predicates for one place (with values 0
// and 3) make the path infeasible, which the refutation detects.
fn guard_temp() {
    let mut i = 0;
    while {
        let t = i;
        t < 3
    } {
        body_invariant!(0 <= i && i < 3);
        i += 1;
    }
    prusti_refute!(false);
}

// `x` is moved out on one branch and still holds its value on the other; the
// join drops that value, guarded by which branch was taken. Inside a loop,
// the guard must reflect the current iteration, not an earlier one.
fn join_drop_in_loop(n: i32) {
    let mut i = 0;
    while i < n {
        body_invariant!(i < n);
        let x = S { v: 1 };
        if i % 2 == 0 {
            consume(x);
        }
        i += 1;
    }
    prusti_refute!(false);
}

// The loop condition moves `x` only when `c` holds, so the value of `x` is
// dropped where the two paths of `&&` join. The condition is encoded twice,
// before the loop head and inside the loop, and each copy must drop the value
// exactly once.
fn while_and_move(c: bool) {
    let mut x = S { v: 0 };
    while c && consume_b(x) {
        body_invariant!(true);
        x = S { v: 1 };
    }
    prusti_refute!(false);
}

// As above, with the value reassigned inside the condition.
fn cond_chain(c: bool, d: bool) {
    let mut x = S { v: 0 };
    while consume_b(x) && { x = S { v: 1 }; c } && d {
        body_invariant!(true);
    }
    prusti_refute!(false);
}

// The arm `0 | 1 | 2` is entered after the guard has moved `z`, and directly
// from the match for 1 and 2, where `z` still holds its value. That edge is
// encoded once per matched value, and the value must still be dropped once.
fn guard_then_shared_arm(k: u8, z: S) -> i32 {
    let r = match k {
        0 if consume_b(z) => 1,
        0 | 1 | 2 => 2,
        _ => 3,
    };
    prusti_refute!(false);
    r
}

pub struct Pair {
    pub x: S,
    pub y: S,
}

fn read(_s: &i32) {}

// `p.x` is moved out on one branch and lent out on the others, where the loans
// expire before the join. After the join, `p.x.v` holds a value only on the
// lending branches, where the value is dropped before the reassignment.
fn move_or_share_or_lend(c: u8) {
    let mut p = Pair { x: S { v: 1 }, y: S { v: 2 } };
    if c == 0 {
        consume(p.x);
    } else if c == 1 {
        read(&p.x.v);
    } else {
        let r = &mut p.x.v;
        *r = 3;
    }
    p.x = S { v: 5 };
    assert!(p.x.v == 5);
    prusti_refute!(false);
}

// `c` holds a value on loop entry and is only overwritten in the loop: the
// invariant keeps its value, which the first write drops.
fn reassign_in_loop() {
    let mut c = 0;
    let mut i = 0;
    while i < 3 {
        body_invariant!(0 <= i && i <= 3);
        c = i;
        i += 1;
    }
    prusti_refute!(false);
}

fn struct_reassign_in_loop() {
    let mut c = S { v: 0 };
    let mut i = 0;
    while i < 3 {
        body_invariant!(0 <= i && i <= 3);
        c = S { v: i };
        i += 1;
    }
    prusti_refute!(false);
}

// Without `body_invariant!`, the invariant is placed at the PCG loop head.
fn reassign_in_loop_no_body_invariant() {
    let mut c = 0;
    let mut i = 0;
    while i < 3 {
        c = i;
        i += 1;
    }
    prusti_refute!(false);
}

fn reassign_in_loop_invariant_at_head() {
    let mut c = 0;
    let mut i = 0;
    loop {
        body_invariant!(0 <= i && i <= 3);
        if i >= 3 {
            break;
        }
        c = i;
        i += 1;
    }
    prusti_refute!(false);
}

fn reassign_in_loop_read_after() {
    let mut c = 0;
    let mut i = 0;
    while i < 3 {
        body_invariant!(0 <= i && i < 3 && (i == 0 || c == i - 1));
        c = i;
        i += 1;
    }
    assert!(c == 2);
    prusti_refute!(false);
}

fn struct_reassign_in_loop_read_after() {
    let mut c = S { v: 0 };
    let mut i = 0;
    while i < 3 {
        body_invariant!(0 <= i && i < 3 && (i == 0 || c.v == i - 1));
        c = S { v: i };
        i += 1;
    }
    assert!(c.v == 2);
    prusti_refute!(false);
}
