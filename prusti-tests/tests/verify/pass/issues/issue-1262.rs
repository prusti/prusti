// Regression test for prusti/prusti#1262: matching a char reference against a char constant.
use prusti_contracts::*;

#[pure]
fn is_tab(c: &char) -> bool {
    matches!(c, '\t')
}

#[pure]
fn is_tab_expanded(c: &char) -> bool {
    match c {
        '\t' => true,
        _ => false,
    }
}

fn check_chars() {
    let tab = '\t';
    let a = 'a';
    prusti_assert!(is_tab(&tab));
    prusti_assert!(!is_tab(&a));
    prusti_assert!(is_tab_expanded(&tab));
}
