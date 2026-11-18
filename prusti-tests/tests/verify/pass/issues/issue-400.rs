// Regression test for prusti/prusti#400: a recursive call passing two shared borrows that expire
// together.
use prusti_contracts::*;

pub struct Tree {
    right: Option<Box<Tree>>,
}

fn helper(node: &Tree, size: &u32) {
    match &node.right {
        None => {}
        Some(r) => {
            helper(r, size);
        }
    }
}
