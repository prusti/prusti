// Regression test for prusti/prusti#400: a recursive call passing two shared borrows that expire
// together.
use prusti_contracts::*;

pub struct Tree {
    right: Option<Box<Tree>>,
}

fn helper_some(node: &Tree, size: &u32) {
    match &node.right {
        None => {}
        Some(r) => {
            helper_some(r, size);
            prusti_assert!(false); //~ERROR: the asserted expression might not hold
        }
    }
}

fn helper_none(node: &Tree, size: &u32) {
    match &node.right {
        None => {
            prusti_assert!(false); //~ERROR: the asserted expression might not hold
        }
        Some(r) => {
            helper_none(r, size);
        }
    }
}

fn helper_after(node: &Tree, size: &u32) {
    match &node.right {
        None => {}
        Some(r) => {
            helper_after(r, size);
        }
    }
    prusti_assert!(*size == 3); //~ERROR: the asserted expression might not hold
}
