use prusti_contracts::*;

trait A { }

#[refine_spec(where T: 'static + A, [ //~ ERROR: lifetimes are not allowed in type-conditional spec refinement trait bounds
    ensures(result > 0)
])]
fn static_lifetime_bound<T>(_x: T) -> i32 {
    42
}

trait B<X> { }

#[refine_spec(where T: for<'a> B<&'a i32> [ //~ ERROR: lifetimes are not allowed in type-conditional spec refinement trait bounds
    ensures(result > 0)
])]
fn higher_ranked_bound<T>(_x: T) -> i32 {
    42
}
