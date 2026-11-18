// Regression test for prusti/prusti#1029: the code reading the reference is encoded, so a false
// claim after it is reported.
use prusti_contracts::*;
use std::collections::BTreeMap;

fn btree_map_get_binding_probe() {
    let mut tree = BTreeMap::<usize, usize>::new();
    let key = 10;
    tree.insert(key, 5);
    let res = tree.get(&key);
    if let Some(a) = res {
        let _b = *a;
        prusti_assert!(false); //~ ERROR: the asserted expression might not hold
    }
}
