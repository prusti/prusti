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
