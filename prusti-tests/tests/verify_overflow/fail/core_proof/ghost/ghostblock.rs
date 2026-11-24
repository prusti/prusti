#![allow(unused)]

use prusti_contracts::*;

fn empty_ghost_block() {
    ghost! {}
}

fn return_disallowed() {
    ghost! {
        return; //~ ERROR: Can't leave the ghost block early
    }
}

fn break_disallowed() {
    while true {
        ghost! {
            break; //~ ERROR: Can't leave the ghost block early
        }
    }
}

fn continue_disallowed(x: bool) {
    while true {
        ghost! {
            if x {
                continue; //~ ERROR: Can't leave the ghost block early
            } else {
                continue; //~ ERROR: Can't leave the ghost block early
            }
        }
    }
}

fn inner_loop_breaks_allowed() {
    ghost! {
        while true {
            break;
            continue;
        }
    }
}

fn inner_labeled_loop_allowed() {
    ghost! {
        'inner: while true {
            break;
        }
    }
}

fn cannot_break_from_for_loop_iterator(v: Vec<u32>, c: bool) {
    loop {
        ghost! {
            for _x in { if c { break; } v.iter() } {} //~ ERROR: Can't leave the ghost block early
        }
    }
}

fn inner_labeled_block_breaks_allowed() {
    ghost! {
        'inner: {
            break 'inner;
        }
    }
}

fn cannot_break_outer_labeled_block() {
    'outer: {
        ghost! {
            while true {
                break 'outer; //~ ERROR: Can't leave the ghost block early
            }
        }
    }
}

fn cannot_break_outer_labeled_loop() {
    'outer: while true {
        ghost! {
            while true {
                continue 'outer; //~ ERROR: Can't leave the ghost block early
                break;
            }
        }
    }
}

fn try_disallowed() -> Option<u32> {
    ghost! {
        let x = Some(5u32)?; //~ ERROR: Can't leave the ghost block early
    };
    None
}

// `return`/`?` inside a closure, async block or item nested in the ghost body
// exit the closure/block/item, not the ghost block.
fn closure_and_item_exits_allowed() {
    ghost! {
        let _f = |x: u32| -> Option<u32> { Some(Some(x)?) };
        let _g = |x: i32| -> i32 { return x + 1; };
        let _h = async { return 1u32; };
        fn helper() -> Option<u32> {
            let x = Some(5u32)?;
            return Some(x);
        }
    };
}
