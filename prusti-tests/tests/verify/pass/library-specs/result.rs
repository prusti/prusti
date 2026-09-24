use prusti_contracts::*;

fn concrete() {
    let ok: Result<i32, bool> = Ok(1);
    assert!(ok.is_ok() && !ok.is_err());
    let err: Result<i32, bool> = Err(false);
    assert!(err.is_err() && !err.is_ok());
}

#[requires(r.is_ok())]
#[ensures(!r.is_err())]
fn in_specs<T, E>(r: &Result<T, E>) {}

#[pure]
#[requires(r.is_ok())]
fn get(r: Result<u32, ()>) -> u32 {
    match r {
        Ok(v) => v,
        Err(()) => unreachable!(),
    }
}
