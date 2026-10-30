use prusti_contracts::*;

#[derive(Clone, Copy)]
pub struct A(usize);

#[derive(Clone, Copy)]
pub struct B([A; 16]);

impl B {
    #[pure]
    /// Initialization of an array holding ADTs.
    pub fn new() -> Self {
        Self([A(0); 16])
    }
}

#[derive(Clone, Copy)]
pub struct BNamed {
    inner: [A; 4]
}

impl BNamed {
    /// Obtain the length of an ADT array.
    #[pure]
    pub const fn len(&self) -> usize {
        self.inner.len()
    }
}
