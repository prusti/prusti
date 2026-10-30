// Issue #47 "Exhaling permission of reassigned `&mut T` argument"

use prusti_contracts::*;

struct T {
    f: i32,
}

#[ensures(x.f == result)]
#[ensures(old(x.f) == result)]
fn extract(x: &mut T) -> i32 {
    // move x
    let y = x;
    let mut z = T { f: 42 };
    let x = &mut z;
    y.f
}

fn rand() -> bool { true }

#[ensures(x.f == result)]
fn extract_simpl(x: &mut T) -> i32 {
    // `x` is moved to `y`
    let y = x;

    if rand() {
        // unfold `y`
        y.f
    } else {
        y.f = 123;
        // `y` is moved to `z`
        let z = y;
        123
    }
    // In Viper, `x` may be an alias of `z` or `y` (which has been unfolded)
}

struct S {
    f: i32
}

#[requires(x.f == 123)]
#[ensures(x.f == 456)]
fn test_move_ref(x: &mut S) {
    let y = S {
        f: 456
    };
    *x = y;
}

#[requires(x.f == 123)]
#[ensures(x.f == 456)]
fn test_replace_arg(x: &mut S) {
    *x = S {
        f: 456
    };
}

impl S {
    #[requires(self.f == 123)]
    #[ensures(self.f == 456)]
    fn test(&mut self) {
        let new_self = S {
            f: 456
        };
        *self = new_self;
    }
}
