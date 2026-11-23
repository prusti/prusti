use prusti_contracts::*;

#[pure]
fn count_up(n: u32) -> u32 { //~ERROR: a loop in pure code
    let mut i = 0;
    while i < n {
        i += 1;
    }
    i
}
