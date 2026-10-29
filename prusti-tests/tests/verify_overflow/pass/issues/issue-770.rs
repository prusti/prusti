use prusti_contracts::*;

pub struct A1 {
    b: isize,
}

impl A1 {
    #[pure]
    #[ensures(result == true)]
    pub const fn invariant(&self) -> bool {
        self.is_negative() != self.is_non_negative()
    }

    #[pure]
    pub const fn is_non_negative(&self) -> bool {
        self.b >= 0
    }

    #[pure]
    pub const fn is_negative(&self) -> bool {
        self.b < 0
    }

    #[requires(self.invariant())]
    pub const fn test(&self) {}
}

pub struct A2 {
    entries: [usize; 4],
}

impl A2 {
    #[pure]
    #[requires(index < 4)]
    #[ensures(0 <= result)]
    pub fn get(&self, index: usize) -> usize {
        self.entries[index]
    }

    pub fn test(&self) {
        self.get(2);
    }
}

pub struct B2 {
    entries: [usize],
}

impl B2 {
    #[pure]
    #[requires(index < self.entries.len())]
    #[ensures(0 <= result)]
    pub fn get(&self, index: usize) -> usize {
        self.entries[index]
    }

    #[requires(self.entries.len() > 2)]
    pub fn test(&self) {
        self.get(2);
    }
}

pub struct A3 {
    b: isize,
}

impl A3 {
    #[pure]
    pub const fn invariant(&self) -> bool {
        self.is_negative() != self.is_non_negative()
    }

    #[pure]
    pub const fn is_non_negative(&self) -> bool {
        self.b >= 0
    }

    #[pure]
    pub const fn is_negative(&self) -> bool {
        self.b < 0
    }

    #[requires(self.invariant())]
    pub const fn test(&self) {}
}

pub struct B4(isize);

impl B4 {
    #[pure]
    #[ensures(result <= 16)]
    pub const fn get_non_negative(&self) -> isize {
        match self.get_saturated() {
            i if i < 0 => 0,
            i => i,
        }
    }

    #[pure]
    pub const fn get_saturated(&self) -> isize {
        match self.0 {
            isize::MIN..=-17 => -16,
            17.. => 16,
            i => i,
        }
    }

    #[requires(_i < self.get_non_negative())]
    pub const fn test(&self, _i: isize) {}
}
