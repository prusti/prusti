// Regression test for prusti/prusti#1029: using a reference returned inside an `Option`.
use std::collections::BTreeMap;

fn btree_map_get_binding() {
    let mut tree = BTreeMap::<usize, usize>::new();
    let key = 10;
    tree.insert(key, 5);
    let res = tree.get(&key);
    match res {
        Some(a) => {
            let _b = *a;
        }
        None => {}
    };
}
