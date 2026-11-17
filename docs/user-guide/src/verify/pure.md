# Pure functions

Pure functions are functions which are deterministic and side-effect free. In Prusti, such functions can be marked with the `#[pure]` attribute. They can take shared references as arguments, but they cannot take mutable references, because modifying the heap is considered a side effect.

At the moment, it is up to the user to ensure that functions annotated with `#[pure]` always terminate. Non-terminating pure functions would allow to infer `false`.

```rust,noplaypen,ignore
use prusti_contracts::*;

#[pure]
#[ensures(result == *a + *b)]
fn pure_example(a: &i32, b: &i32) -> i32 {
  *a + *b
}

#[ensures(*c == a + b)]
fn impure_example(a: &i32, b: &i32, c: &mut i32) {
  *c = *a + *b
}
```

## Values and memory addresses

The result of a `#[pure]` function may depend only on the *values* of its arguments. For a shared-reference argument, only the value it points to matters, not where that value is in memory: the result of `pure_example(&x, &y)` is the same in a specification as in executable code, and the same for any two pairs of references to equal values. A `#[pure]` function whose result depends on where an argument is stored, for example because it returns a reference into an argument, is rejected:

```rust,noplaypen,ignore
use core::ops::Bound;
use prusti_contracts::*;

struct Interval { lo: usize, hi: usize }

impl Interval {
    #[pure]
    fn start_bound(&self) -> Bound<&usize> {
        Bound::Included(&self.lo)
    }
}
```

Here, Prusti reports that the result of `Interval::start_bound` might depend on the addresses of its shared-reference arguments.

The *mode* `memory` lets the result also depend on the memory addresses of the arguments. Callers then learn the whole result, addresses included, but two calls on different objects with equal contents may give different results:

```rust,noplaypen,ignore
impl Interval {
    #[pure(memory)]
    fn start_bound(&self) -> Bound<&usize> {
        Bound::Included(&self.lo)
    }
}

fn client(i: &Interval) {
    if let Bound::Included(lo) = i.start_bound() {
        prusti_assert!(*lo == i.lo);
        prusti_assert!(lo as *const usize == &i.lo as *const usize);
    }
}
```

A `#[pure]` function is also a valid `#[pure(memory)]` function, so an implementation of a `#[pure(memory)]` trait method may be `#[pure]`, but an implementation of a `#[pure]` trait method may not be `#[pure(memory)]`.

Prusti does not check the bodies of `#[trusted]` functions. A trusted function returning a reference into an argument should therefore be declared `#[pure(memory)]`: as a `#[pure]` function, Prusti would assume that it returns the same reference for any two arguments with equal values.

A known limitation: a specification does not yet see the addresses of the local variables it borrows, so `&x` for a local `x` is not at the address of `x` there. A `#[pure(memory)]` function applied to `&x` in a specification may therefore give a different result than the same call in executable code. Calls on references that the enclosing function receives as arguments agree.

Currently, Prusti ignores only the addresses of arguments whose declared type is a shared reference. Addresses stored inside an argument, for example in a `&&T` or in a struct holding a reference, and the address of an argument of a generic type instantiated with a reference are still visible to a `#[pure]` function.
