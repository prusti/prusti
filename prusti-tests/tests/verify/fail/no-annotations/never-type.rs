#![feature(never_type)]

fn diverging() -> ! {
    panic!();  //~ERROR: might not hold
}
