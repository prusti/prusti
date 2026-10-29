use prusti_contracts::*;

#[derive(Clone, Copy)]
pub struct A1;

impl A1 {
    #[pure]
    pub fn get(&self) -> A1 {
        A1
    }
}

#[pure]
pub fn test1(a: A1) -> A1 {
    a.get().get()
}

#[derive(Clone, Copy)]
pub struct A2;

impl A2 {
    #[pure]
    pub fn get(&self) -> A2 {
        A2
    }

    #[pure]
    pub fn len(&self) -> usize {
        0
    }
}

#[pure]
pub fn test2(a: A2) -> usize {
    a.get().len()
}

#[derive(Clone, Copy)]
pub struct A3;

impl A3 {
    #[pure]
    pub fn len(&self) -> usize {
        0
    }
}

#[pure]
pub fn a3() -> A3 {
    A3
}

#[pure]
pub fn test3() -> usize {
    a3().len()
}

#[derive(Clone, Copy)]
pub struct A4(usize);

impl A4 {
    #[pure]
    pub fn len(&self) -> usize {
        0
    }
}

#[pure]
pub fn a4() -> A4 {
    A4(0)
}

#[pure]
pub fn test4() -> usize {
    a4().len()
}
