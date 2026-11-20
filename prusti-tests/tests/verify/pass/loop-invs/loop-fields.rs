use prusti_contracts::*;

struct T {
    f: u32,
}

struct H {
    g: T,
}

#[requires(a.g.f < 5)]
#[ensures(result.f == 5)]
fn test1(a: H) -> T {
    let mut a = a;
    let mut cont = true;
    while cont {
        body_invariant!(a.g.f < 5);
        body_invariant!(cont == (a.g.f < 5));
        a.g.f += 1;
        cont = a.g.f < 5;
    }
    a.g
}

// The loop reads a field behind the shared reference `a` while `r` borrows it.
#[ensures(result >= 3)]
fn test2(a: &H) -> u32 {
    let r = &a.g.f;
    let mut i = 0;
    while i < 3 {
        body_invariant!(*r == a.g.f);
        let _x = *r;
        let _y = a.g.f;
        i += 1;
    }
    i
}
