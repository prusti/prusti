use prusti_contracts::*;

#[require(x!=0)] //~ERROR: cannot find attribute
#[ensures(result!=0)]
fn wrong(x: i32) -> i32 {
    x
}

#[pure(values)] //~ERROR: `#[pure]` takes no argument or the mode `memory`
fn wrong_pure_mode(x: &i32) -> i32 {
    *x
}

#[pure]
#[pure(memory)] //~ERROR: `#[pure]` may be given only once
fn twice_pure(x: &i32) -> &i32 {
    x
}

fn wrong_closure_pure_mode() {
    let _f = closure!(
        #[pure(memory)] //~ERROR: a mode of `#[pure]` is not supported on `closure!`
        |x: i32| -> i32 { x }
    );
}
