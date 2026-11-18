//@ compile-flags: -Penable_type_invariants=true

// The type attributes generate an inherent impl of the type, which must
// repeat its lifetime, type and const parameters and its where-clause.

use prusti_contracts::*;

#[trusted]
struct TrustedRef<'a> {
    r: &'a i32,
}

#[trusted]
struct TrustedArray<const N: usize> {
    a: [i32; N],
}

#[trusted]
struct TrustedBounded<T>
where
    T: Copy,
{
    t: T,
}

#[trusted]
struct TrustedDefault<T = i32> {
    t: T,
}

#[invariant(*self.r > 0)]
struct InvariantRef<'a> {
    r: &'a i32,
}

#[invariant(N > 0)]
struct InvariantArray<const N: usize> {
    a: [i32; N],
}

#[invariant(true)]
struct InvariantBounded<T>
where
    T: Copy,
{
    t: T,
}

#[print_counterexample("r = {}", r)]
struct CounterexampleRef<'a> {
    r: &'a i32,
}

#[print_counterexample("a = {}", a)]
struct CounterexampleArray<const N: usize> {
    a: [i32; N],
}

#[print_counterexample]
enum CounterexampleBounded<T>
where
    T: Copy,
{
    #[print_counterexample("t = {}", t)]
    A { t: T },
    B,
}
