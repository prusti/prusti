use prusti_contracts::*;

// A reborrow rebuilds the referent's snapshot from its concrete value.

#[ensures(result === x)]
fn reborrow<T>(x: &T) -> &T {
    &*x
}

#[derive(Clone, Copy)]
struct S {
    f: u32,
}

#[ensures(result === x)]
fn reborrow_concrete(x: &S) -> &S {
    &*x
}

#[requires(x.f == 3)]
fn client(x: &S) {
    let r = reborrow(x);
    let r2: &S = &*r;
    prusti_assert!(r2 === x);
    assert!(r2.f == 3);
}

pub enum Link<T> {
    None,
    Some(Box<Node<T>>),
}

pub struct Node<T> {
    elem: T,
    next: Link<T>,
}

// `node` dies without the encoding knowing that `Box<Node<T>>` is inhabited.
fn dead_binding<T>(link: &Link<T>) -> usize {
    match link {
        Link::None => 0,
        Link::Some(node) => {
            let _copy = node;
            1
        }
    }
}

fn dead_dereferenced_binding<T>(link: &Link<T>) -> usize {
    match link {
        Link::None => 0,
        Link::Some(node) => {
            let _next = &node.next;
            1
        }
    }
}
