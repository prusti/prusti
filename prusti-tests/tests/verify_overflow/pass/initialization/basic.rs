#![feature(nll)]
#![feature(box_patterns)]

use prusti_contracts::*;

fn test3() {
    let x = 5;
    if false {
        let y = 4;
        assert!(y == 4);
    }
    let z = 3;
    assert!(x == 5);
    assert!(z == 3);
}

// Reassignment

struct InfiniteList1 {
    next: Box<InfiniteList1>
}

fn consume1(x: InfiniteList1) {}

fn reassignment1(mut x: InfiniteList1) {
    // Consume the only path.
    x = *(*(*(*(*x.next).next).next).next).next;
    consume1(x);
}

struct InfiniteList2 {
    val: u32,
    next: Box<InfiniteList2>
}

fn consume2(x: InfiniteList2) {}

fn reassignment2a(a: InfiniteList2) {
    let mut x = a;
    // Consume one of the paths.
    x = *x.next;
    consume2(x);
}

fn reassignment2b(a: InfiniteList2) {
    let mut x = a;
    // Consume one of the paths.
    x = *(*(*(*(*x.next).next).next).next).next;
    consume2(x);
}
