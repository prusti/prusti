use prusti_contracts::*;

struct S {
    f: i32
}

impl S {
    #[requires(self.f == 123)]
    #[ensures(self.f == 123)]
    pub fn test(self) {
        let x = 123;
        assert!(self.f == x);
    }
}

struct S2 {
    pub f: i32
}

impl S2 {
    #[requires(f == 123)]
    #[ensures(result.f == 123)]
    fn from(f: i32) -> S2 {
        S2 { f }
    }
}
