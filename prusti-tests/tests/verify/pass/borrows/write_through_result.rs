//@ known-failure
// A call's result holding a mutable borrow is written through and then dies.
// The write labels the result's lifetime projection, whose expansion is only
// forgotten on removal, so the call's wand misses its indirect predicate.

struct S<'a>(&'a mut i32);

fn mk(r: &mut i32) -> S<'_> {
    S(r)
}

fn struct_field() {
    let mut x = 1;
    {
        let s = mk(&mut x);
        *s.0 = 3;
    }
    let _y = x;
}

fn boxed() {
    let mut x = 1;
    {
        let b = Box::new(&mut x);
        **b = 3;
    }
    let _y = x;
}
