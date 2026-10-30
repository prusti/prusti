//@ compile-flags: -Penable_type_invariants=true
use prusti_contracts::*;

// precondition assert

#[invariant(self.value <= 100)]
struct Percentage {
    value: u8,
}

impl Percentage {
    fn incr(&mut self) {
        if self.value < 100 {
            self.value += 1;
        }
    }
}

#[requires(x <= 100)]
fn test(x: u8) {
    let mut perc = Percentage { value: x };
    perc.incr();
}

// postcondition (result) assert

#[invariant(self.value <= 100)]
struct PercentageNew {
    value: u8,
}

impl PercentageNew {
    #[requires(value <= 100)]
    fn new(value: u8) -> Self {
        PercentageNew {
            value: value,
        }
    }
}

// construction

#[invariant(self.value <= 100)]
struct PercentageLiteral {
    value: u8,
}

fn make_percentage() -> PercentageLiteral {
    PercentageLiteral { value: 99 }
}
