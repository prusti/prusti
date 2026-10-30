#![feature(box_patterns)]

fn create_box(v: i32) -> Box<i32> {
    let x = Box::new(v);
    let y = *x;
    assert!(v == y);
    let z = Box::new(y);
    assert!(v == *z);
    Box::new(*z)
}

fn use_box(x: Box<i32>) -> i32 {
    *x
}

struct Tree {
    val: i32,
    left: Option<Box<Tree>>,
    right: Option<Box<Tree>>,
}

fn take_left(tree: Tree) -> Option<Tree> {
    match tree.left {
        Some(box left) => Some(left),
        None => None,
    }
}

struct InfiniteList {
    val: i32,
    next: Box<InfiniteList>
}

fn consume(x: InfiniteList) {}

fn reassignment(a: InfiniteList) {
    let mut x = a;

    // Reassignment
    x = *(*(*(*(*x.next).next).next).next).next;

    consume(x);
}
