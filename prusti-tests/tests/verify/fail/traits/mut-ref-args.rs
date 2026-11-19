use prusti_contracts::*;

// A call of a trait method goes to its stub, which takes the targets of the
// mutable references in its arguments and gives them back, possibly changed.

trait Set {
    fn set(&mut self);
}

impl Set for i32 {
    fn set(&mut self) {
        *self = 1;
    }
}

fn direct() {
    let mut c = 0;
    c.set();
    assert!(c == 0); //~ERROR: might not hold
}

// The stub's signature has no regions for references inside a type parameter,
// so it does not take their targets, and an implementation that needs them
// is rejected.
impl Set for &mut i32 {
    fn set(&mut self) { //~ERROR: the implementation needs the targets of mutable references
        **self = 1;
    }
}

// Likewise for a reference held in a field of `Self`.
pub struct Counter<'a> {
    count: &'a mut u32,
}

impl<'a> Iterator for Counter<'a> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> { //~ERROR: the implementation needs the targets of mutable references
        *self.count += 1;
        None
    }
}

// A call of the trait method gives back the wands of its signature, with its
// pledges, which an implementation that replaces them is not checked against.
pub struct Field {
    f: i32,
}

pub trait Get {
    #[after_expiry(n == 5)]
    fn get(&mut self, n: i32) -> &mut i32;
}

#[refine_trait_spec]
impl Get for Field {
    #[after_expiry(true)]
    fn get(&mut self, n: i32) -> &mut i32 { //~ERROR: refining the pledges of a trait method is not supported
        &mut self.f
    }
}
