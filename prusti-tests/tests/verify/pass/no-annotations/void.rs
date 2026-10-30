#![feature(never_type)]

pub enum Void { }

pub fn unreachable(x: Void) -> ! {
    match x {}
}

fn void_unwrap<T>(this: Result<T, Void>) -> T {
    match this {
        Ok(val) => val,
        Err(e) => unreachable(e)
    }
}

pub fn unreachable_box(x: Box<Void>) -> ! {
    match *x {}
}

fn void_unwrap_box<T>(this: Result<T, Box<Void>>) -> T {
    match this {
        Ok(val) => val,
        Err(e) => unreachable_box(e)
    }
}

pub fn unreachable_mut(x: &mut Void) -> ! {
    match *x {}
}

pub fn unreachable_ref(x: &Void) -> ! {
    match *x {}
}

mod declaring {
    use super::Void;

    pub struct SecretlyVoid {
        _void: Void,
    }

    pub enum Either<T> {
        Value(T),
        Empty(SecretlyVoid),
    }
}

pub fn unwrap_outside<T>(this: declaring::Either<T>) -> T {
    match this {
        declaring::Either::Value(value) => value,
        declaring::Either::Empty(_) => unreachable!(),
    }
}

fn diverging() -> ! {
    diverging()
}

fn test() {
    diverging();
    // The return type is `Never`, so Prusti should assume `false` here.
    // The following assertion should not raise verification errors
    assert!(1 == 2);
}
