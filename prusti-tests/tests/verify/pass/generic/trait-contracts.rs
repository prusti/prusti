use prusti_contracts::*;

trait FooDefault {
    #[requires(x < 10)]
    #[ensures(result > x)]
    fn bar(x: u32) -> u32 {
        x + 1
    }
}

impl FooDefault for bool {}

#[ensures(result)]
fn client() -> bool {
    let x = 5;
    let y = bool::bar(x);
    y > x
}

trait FooPost: Sized {
    #[ensures(result == 5)]
    fn baz(self) -> u32;
}

impl FooPost for u32 {
    fn baz(self) -> u32 {
        5
    }
}

#[ensures(result == 5)]
fn go_post<T: FooPost>(t: T) -> u32 {
    t.baz()
}

trait FooPre: Sized {
    #[requires(x > 5)]
    fn baz(self, x: u32) -> u32;
}

impl FooPre for u32 {
    fn baz(self, x: u32) -> u32 {
        check(x);
        0
    }
}

#[requires(x > 2)]
fn check(x: u32) {
}


fn go_pre<T: FooPre>(t: T) -> u32 {
    t.baz(7)
}
