use prusti_contracts::*;


#[ensures(result == 6)] //~ERROR: postcondition might not hold
fn func() -> u32{
    5
}

#[ensures(result == old(x))] //~ERROR: postcondition might not hold
fn identity(x: i32) -> i32 {
    x + 1
}

struct S {
    f: i32
}

impl S {
    #[requires(true)]
    #[ensures(false)] //~ERROR: postcondition
    pub fn test(self) {}
}

#[ensures(forall(|y: u32| forall(|z: u32| y + z > old(*x))))] //~ERROR: postcondition might not hold
fn test(x: &mut u32) {}
