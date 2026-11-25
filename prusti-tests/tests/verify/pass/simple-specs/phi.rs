use prusti_contracts::*;

#[pure]
#[requires(0 <= b && b < 100)]
#[requires(0 <= c && c < 100)]
fn test(a: bool, b: i32, c: i32) -> i32 {
    let mut x = b;
    let mut y = c;
    if a {
        x = c;
        y = b;
    }
    x + 2 * y
}

// Only one branch assigns the argument, so the other one keeps its value from
// before the branch.
#[pure]
fn assign_arg_in_one_branch(c: bool, mut a: u32, b: u32) -> u32 {
    if c { a = b; }
    a
}

#[ensures(test(false, 42, 72) == 186)]
#[ensures(test(true, 42, 72) == 156)]
#[ensures(assign_arg_in_one_branch(false, 1, 2) == 1)]
#[ensures(assign_arg_in_one_branch(true, 1, 2) == 2)]
fn test_branch_results() {}
