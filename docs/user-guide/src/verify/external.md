# External specifications

Since the Rust standard library and external libraries do not specify contracts for their functions, Prusti allows specifying the contract for functions separately from where they are implemented. Such a specification looks like a regular `impl`, with the exception that there are no bodies in the implementation functions, and that the `impl` has an `#[extern_spec]` attribute.

The standard library type `std::option::Option` could be specified as follows:

```rust,noplaypen,ignore
use prusti_contracts::*;

#[extern_spec]
impl<T> std::option::Option<T> {
    #[trusted]
    #[pure]
    #[ensures(matches!(*self, Some(_)) == result)]
    pub fn is_some(&self) -> bool;

    #[trusted]
    #[pure]
    #[ensures(self.is_some() == !result)]
    pub fn is_none(&self) -> bool;

    #[trusted]
    #[requires(self.is_some())]
    pub fn unwrap(self) -> T;

    // ...
}
```

Prusti does not verify the functions of an external specification against their implementation, so every function in an external specification must be marked [`#[trusted]`](trusted.md). It is possible to specify multiple `#[extern_spec]` implementations for the same type, but it is an error to externally specify the same function multiple times.

An external specification applies at every use of the function it specifies. So its type and const parameters must be those of that function, in the same order: `impl<E, T> std::result::Result<T, E>` and `impl std::option::Option<i32>` are rejected, while lifetimes may be named differently or elided. Likewise, every trait bound it declares must follow from the bounds of that function. For example, `impl<T: Copy> std::option::Option<T>` is rejected for `is_some`, which has no `Copy` bound, and a type parameter that the function declares `?Sized` must be `?Sized` in the specification too. The specification has to declare the bounds that the specified function needs, but may leave out those that follow from them, such as the supertraits of a trait.

In a specification of a trait, `Self` is `?Sized` unless the trait or the method requires otherwise, as in the trait itself. So a method declared with `where Self: Sized` (such as `Iterator::by_ref`) needs it in the specification too. A method that takes or returns a value whose size depends on `Self`, such as `Self`, `(u8, Self)` or `Cell<Self>`, needs `where Self: Sized` in its specification, just like a specification on a method of a local trait. It is added automatically for `self`, `Self` and a tuple ending in `Self`, and has to be written out otherwise. A method that only takes `&self` must not declare `where Self: Sized`, so its specification cannot pass `self` to a helper function that requires `Sized`: declare the helper's type parameter `?Sized` instead.

The `extern_spec` attribute accepts an optional argument to provide the module path to the function being specified. For example, to specify `std::mem::swap`, the argument is `std::mem`:

```rust,noplaypen,ignore
use prusti_contracts::*;

#[extern_spec(std::mem)]
#[trusted]
#[ensures(*a === *old(&*b) && *b === *old(&*a))]
fn swap<T>(a: &mut T, b: &mut T);
```
