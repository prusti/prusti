use prusti_contracts::*;

#[feature(nll)]
struct G {
    value: u32,
}

struct F {
    f: G,
    g: G,
}

fn use_mut_ref(x: &mut F) {}

fn take(_x: &mut Box<i32>) {}

fn test() {
    let mut x = Box::new(123);
    loop {
        let y = &mut x;
        take(y);
    }
}

struct T(u32);

fn random() -> bool { true }

fn consume(y: &mut T) { }

fn check_join(mut x: (T, T)) -> T {
    // We have both `x.0` and `x.1`
    if random() {
        // We move permission of `x.0` to consume
        consume(&mut x.0)
    }
    // After the join we would like to use `x.1`
    x.1
}

fn reborrow<T>(x: &mut T) -> &mut T {
    x
}

pub fn test1() {
    let mut a = 6;
    let x = reborrow(&mut a);
    *x = 4;
}
