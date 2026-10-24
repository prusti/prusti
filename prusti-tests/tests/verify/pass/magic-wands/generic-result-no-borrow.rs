use prusti_contracts::*;

// The result has type `T`, which cannot borrow from `dest`, so the referent
// of `dest` is given back directly rather than through a wand.

#[trusted]
#[ensures(*dest === src)]
#[ensures(result === *old(&*dest))]
fn replace<T>(dest: &mut T, src: T) -> T {
    std::mem::replace(dest, src)
}

#[ensures(*dest === src)]
#[ensures(result === *old(&*dest))]
fn replace_wrapper<T>(dest: &mut T, src: T) -> T {
    replace(dest, src)
}

fn client() {
    let mut x = 1;
    let y = replace_wrapper(&mut x, 2);
    assert!(x == 2 && y == 1);
    let z = replace(&mut x, 3);
    assert!(x == 3 && z == 2);
}
