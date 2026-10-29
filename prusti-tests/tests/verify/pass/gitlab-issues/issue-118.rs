#![feature(nll)]
#![feature(box_patterns)]

use prusti_contracts::*;

struct List {
    next: Box<List>,
}

fn consume(head: &mut List) {
}

fn recursive(head: &mut List, b: bool) {
    let x = if b {
        let box ref mut int_next = head.next;
        int_next
    } else {
        head
    };
    consume(x);
}

struct ValueList {
    value: u32,
    next: Option<Box<ValueList>>,
}

fn recursive_get_last(head: &mut ValueList) -> &mut u32 {
    match head.next {
        None => &mut head.value,
        Some(box ref mut next_ref) => recursive_get_last(next_ref)
    }
}
