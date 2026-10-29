use prusti_contracts::*;

#[derive(Clone, Copy)]
pub struct A2(usize);

#[derive(Clone, Copy)]
pub struct B2([A2; 16]);

impl B2 {
    /// Lookup an ADT from an array
    #[requires(index < self.0.len())]
    pub const fn get(&self, index: usize) -> A2 {
        self.0[index]
    }

    /// Lookup an ADT from an array
    #[pure]
    #[requires(index < self.0.len())]
    pub const fn get_pure(&self, index: usize) -> A2 {
        self.0[index]
    }
}

#[derive(Clone, Copy)]
pub struct A3(usize);

#[derive(Clone, Copy)]
pub struct B3([A3; 16]);

impl B3 {
    /// Assign an ADT to an array element directly
    #[requires(index < self.0.len())]
    pub fn set(&mut self, index: usize, a: A3) {
        self.0[index] = a;
    }
}

#[derive(Clone, Copy)]
pub struct A6(usize);

#[derive(Clone, Copy)]
pub struct B6([A6; 4]);

impl B6 {
    /// Initialization of an array holding ADTs.
    #[pure]
    pub const fn new() -> Self {
        Self([A6(0), A6(1), A6(2), A6(3)])
    }
}

#[derive(Clone, Copy)]
pub struct A8(usize);

pub struct B8 {
    inner: [A8],
}

impl B8 {
    #[pure]
    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    /// Lookup an ADT from a slice
    #[requires(index < self.len())]
    pub const fn get(&self, index: usize) -> A8 {
        self.inner[index]
    }

    /// Lookup an ADT from a slice
    #[pure]
    #[requires(index < self.len())]
    pub const fn get_pure(&self, index: usize) -> A8 {
        self.inner[index]
    }
}

#[derive(Clone, Copy)]
pub struct A9(usize);

pub struct B9 {
    inner: [A9],
}

impl B9 {
    #[pure]
    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    /// Assign an ADT to a slice element directly
    #[requires(index < self.len())]
    pub fn set(&mut self, index: usize, a: A9) {
        self.inner[index] = a;
    }
}
