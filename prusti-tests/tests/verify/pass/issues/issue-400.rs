use prusti_contracts::*;

// Regression test for prusti/prusti#400.
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
