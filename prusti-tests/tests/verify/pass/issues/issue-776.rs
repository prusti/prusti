use prusti_contracts::*;

// Regression test for prusti/prusti#776: match on a tuple of shared references to enums.
#[pure]
fn is_some(o: &Option<i32>) -> bool {
    match o {
        Some(_) => true,
        None => false,
    }
}

#[ensures(result == 1 ==> is_some(l) && is_some(r))]
#[ensures(result == 2 ==> !is_some(l) && !is_some(r))]
#[ensures(result == 3 ==> is_some(l) != is_some(r))]
#[ensures(result == 1 || result == 2 || result == 3)]
fn kind(l: &Option<i32>, r: &Option<i32>) -> u8 {
    match (l, r) {
        (Some(_), Some(_)) => 1,
        (None, None) => 2,
        _ => 3,
    }
}

fn caller() {
    let a = Some(1);
    let b: Option<i32> = None;
    let k = kind(&a, &b);
    prusti_assert!(k == 3);
}
