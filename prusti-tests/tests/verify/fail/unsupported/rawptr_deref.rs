use prusti_contracts::*;

fn read(p: *const i32) -> i32 {
    unsafe { *p } //~ERROR: dereference of the raw pointer `*const i32`
}

fn write(p: *mut i32) {
    unsafe {
        *p = 6; //~ERROR: dereference of the raw pointer `*mut i32`
    }
}

#[requires(unsafe { *p } == 3)] //~ERROR: dereference of the raw pointer `*const i32`
fn spec(p: *const i32) {}

// If one spec cannot be encoded, all functional specs are skipped: callers
// need not establish the other preconditions, and do not get the
// postconditions.
#[requires(unsafe { *p } == 3)] //~ERROR: dereference of the raw pointer `*const i32`
#[requires(n > 0)]
#[ensures(result == n)]
fn partial_spec(p: *const i32, n: u32) -> u32 {
    n
}

fn partial_spec_caller(p: *const i32) {
    partial_spec(p, 0);
    let r = partial_spec(p, 1);
    prusti_assert!(r == 1); //~ERROR: the asserted expression might not hold
}

fn switch(p: *const u32) -> u32 {
    unsafe {
        match *p { //~ERROR: dereference of the raw pointer `*const u32`
            0 => 1,
            _ => 2,
        }
    }
}

#[pure]
fn switch_pure(p: *const u32) -> u32 {
    unsafe {
        match *p {
            //~^ ERROR: dereference of the raw pointer `*const u32`
            0 => 1,
            _ => 2,
        }
    }
}

// The trait's skipped preconditions are no hypothesis to check the impl's
// against, so the impl's precondition is not reported as stronger.
trait Tr {
    #[requires(unsafe { *p } == 3)] //~ERROR: dereference of the raw pointer `*const i32`
    #[requires(n > 0)]
    fn f(&self, p: *const i32, n: u32);
}

struct S;

#[refine_trait_spec]
impl Tr for S {
    #[requires(n > 0)]
    fn f(&self, p: *const i32, n: u32) {}
}
