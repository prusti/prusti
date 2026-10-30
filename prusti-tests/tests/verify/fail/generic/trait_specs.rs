use prusti_contracts::*;

trait FooDefault {
    #[ensures(false)] //~ ERROR: postcondition might not hold
    fn bar(){

    }
}

trait FooPost: Sized {
    #[ensures(result == 5)] //~ ERROR: postcondition might not hold
    fn baz(self) -> u32;
}

impl FooPost for u32 {
    fn baz(self) -> u32 {
        4
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

fn go_pre<T: FooPre>(t: T) -> u32 {
    t.baz(2) //~ ERROR: precondition might not hold
}
