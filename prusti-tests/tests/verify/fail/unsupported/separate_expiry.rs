// Borrows in the result that expire separately but depend on a common argument
// are not supported: the function has no sound permission contract, so it is
// not encoded, and neither is any caller. The function and each call, whether
// its caller is encoded (in source order) before or after the function, and
// whether or not the borrows expire, are reported.

pub fn caller_before() {
    let mut a = 1;
    let mut b = 2;
    let (ra, rb) = two(&mut a, &mut b); //~ERROR: cannot encode function body `caller_before`: borrows in the result that expire separately but depend on a common argument are not supported
    *ra = 3;
    *rb = 4;
}

pub fn two<'a, 'b, 'c>(x: &'a mut u32, y: &'b mut u32) -> (&'c mut u32, &'b mut u32) //~ERROR: cannot encode method `two`: borrows in the result that expire separately but depend on a common argument are not supported
where
    'a: 'c,
    'b: 'c,
{
    (x, y)
}

pub fn caller_after() {
    let mut a = 1;
    let mut b = 2;
    let (ra, rb) = two(&mut a, &mut b); //~ERROR: cannot encode function body `caller_after`: borrows in the result that expire separately but depend on a common argument are not supported
    *ra = 3;
    *rb = 4;
}

fn sink(_: &mut u32, _: &mut u32) -> ! {
    loop {}
}

pub fn caller_without_expiry() {
    let mut a = 1;
    let mut b = 2;
    let (ra, rb) = two(&mut a, &mut b); //~ERROR: cannot encode function body `caller_without_expiry`: borrows in the result that expire separately but depend on a common argument are not supported
    sink(ra, rb);
}

// A trait method without a body is reported as well.
pub trait T {
    fn two<'a, 'b, 'c>(&self, x: &'a mut u32, y: &'b mut u32) -> (&'c mut u32, &'b mut u32) //~ERROR: cannot encode method `T::two`: borrows in the result that expire separately but depend on a common argument are not supported
    where
        'a: 'c,
        'b: 'c;
}

pub fn trait_caller<X: T>(t: &X) { //~ERROR: cannot encode function body `trait_caller`: borrows in the result that expire separately but depend on a common argument are not supported
    let mut a = 1;
    let mut b = 2;
    let (ra, rb) = t.two(&mut a, &mut b);
    *ra = 3;
    *rb = 4;
}
