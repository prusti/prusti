//@ known-failure
// Unsizing coercions of values that hold mutable borrows, which the borrowed
// places get back once the coerced value dies.

trait Tr {
    fn poke(&mut self);
}

struct S<'a>(&'a mut i32);

impl<'a> Tr for S<'a> {
    fn poke(&mut self) {}
}

fn slice_of_refs(_v: &mut [&mut i32]) {}

fn dyn_arg(_t: &mut dyn Tr) {}

fn shared_slice_of_refs(_v: &[&mut i32]) {}

fn box_slice_of_refs(_v: Box<[&mut i32]>) {}

fn array_to_slice() {
    let mut x = 1;
    let mut a = [&mut x];
    slice_of_refs(&mut a);
    let _y = x;
}

fn struct_to_dyn() {
    let mut x = 1;
    let mut s = S(&mut x);
    dyn_arg(&mut s);
    let _y = x;
}

fn shared_array_to_slice() {
    let mut x = 1;
    let a = [&mut x];
    shared_slice_of_refs(&a);
    let _y = x;
}

fn box_array_to_slice() {
    let mut x = 1;
    let b: Box<[&mut i32; 1]> = Box::new([&mut x]);
    box_slice_of_refs(b);
    let _y = x;
}
