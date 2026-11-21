// A blanket impl's associated types and method specifications are assumed
// only where the impl's where-clauses hold (see
// `verify/fail/traits/blanket_*.rs`). These check that they still resolve
// where the bounds do hold: for concrete types, and for generic code whose
// own where-clauses provide them.

use prusti_contracts::*;
use std::convert::{Infallible, TryFrom};

// `impl<T, U: Into<T>> TryFrom<U> for T` with `Error = Infallible`.
pub fn try_from_same(x: u8) -> u8 {
    let r: Result<u8, Infallible> = u8::try_from(x);
    match r {
        Ok(v) => v,
        Err(e) => match e {},
    }
}

pub fn try_from_generic<T, U: Into<T>>(u: U) -> T {
    match T::try_from(u) {
        Ok(t) => t,
        Err(e) => match e {},
    }
}

pub trait MyBound {}
pub trait Tr<U> {
    type Out;
    #[pure]
    fn make(&self, u: U) -> Self::Out;
}

pub struct S;
#[derive(Clone, Copy)]
pub struct Z(pub u32);

impl MyBound for Z {}

#[refine_trait_spec]
impl<U: MyBound + Copy> Tr<U> for S {
    type Out = U;
    #[pure]
    fn make(&self, u: U) -> U {
        u
    }
}

#[pure]
pub fn via<T: Tr<V>, V>(t: &T, v: V) -> T::Out {
    t.make(v)
}

// `<S as Tr<Z>>::Out == Z` and the impl's `make` spec, at a concrete type.
pub fn concrete_user(z: Z) {
    let o: Z = via(&S, z);
    prusti_assert!(o.0 == z.0);
}

// The same, where `V: MyBound + Copy` is only known from the where-clause.
#[pure]
#[ensures(result === v)]
pub fn generic_user<V: MyBound + Copy>(v: V) -> V {
    via(&S, v)
}

// `impl<I: Iterator> IntoIterator for I`: `IntoIter == I`.
pub fn into_iter_is_identity<I: Iterator<Item = u32>>(i: I) -> I {
    i.into_iter()
}

// The behavioural-subtyping check of an impl assumes the impl's
// where-clauses: `via(&self.0) == 7` needs `T: Copy`.
mod subtyping_check_bounds {
    use prusti_contracts::*;

    pub trait Seven {
        #[pure]
        fn seven(&self) -> u32;
    }
    #[refine_trait_spec]
    impl<T: Copy> Seven for T {
        #[pure]
        #[ensures(result == 7)]
        fn seven(&self) -> u32 {
            7
        }
    }

    #[pure]
    pub fn via<T: Seven>(t: &T) -> u32 {
        t.seven()
    }

    pub trait Tr {
        #[ensures(result == 7)]
        fn f(&self) -> u32;
    }
    pub struct W<T>(pub T);
    #[refine_trait_spec]
    impl<T: Copy> Tr for W<T> {
        #[ensures(result == via(&self.0))]
        fn f(&self) -> u32 {
            via(&self.0)
        }
    }
}

// An impl parameter fixed only by a closure bound's output (`U`, as in
// itertools' `MapSpecialCaseFnOk`) that occurs in an item's signature, and so
// in the trigger of the item's axioms.
mod fn_output_param {
    use prusti_contracts::*;

    pub trait MapFn<T> {
        type Out;
        fn call(&mut self, t: T) -> Self::Out;
    }

    pub struct MapOk<F>(pub F);

    #[refine_trait_spec]
    impl<F, T, U, E> MapFn<Result<T, E>> for MapOk<F>
    where
        F: FnMut(T) -> U,
    {
        type Out = Result<U, E>;
        #[trusted]
        fn call(&mut self, t: Result<T, E>) -> Result<U, E> {
            t.map(&mut self.0)
        }
    }

    pub fn go<M: MapFn<Result<u8, ()>>>(m: &mut M, t: Result<u8, ()>) -> M::Out {
        m.call(t)
    }

    pub fn user<F: FnMut(u8) -> u16>(m: &mut MapOk<F>) -> Result<u16, ()> {
        go(m, Ok(1))
    }
}

// An impl forwarding to a projection of its own parameter (as rand_core's
// `TryRng` for `R: DerefMut`) resolves through `Deref` impls, also where only
// the projection at the concrete type occurs (`<&mut S as TryRng>::Error` in
// `user`). Its axioms form a known matching loop (see `guarded_forall_in`).
mod forward_to_projection {
    use prusti_contracts::*;
    use std::ops::DerefMut;

    pub trait TryRng {
        type Error;
        fn try_next(&mut self) -> Result<u32, Self::Error>;
    }

    #[refine_trait_spec]
    impl<R: DerefMut> TryRng for R
    where
        R::Target: TryRng,
    {
        type Error = <R::Target as TryRng>::Error;
        #[trusted]
        fn try_next(&mut self) -> Result<u32, Self::Error> {
            self.deref_mut().try_next()
        }
    }

    pub struct S;
    #[refine_trait_spec]
    impl TryRng for S {
        type Error = ();
        #[trusted]
        fn try_next(&mut self) -> Result<u32, ()> {
            Ok(4)
        }
    }

    pub fn take<R: TryRng>(r: &mut R) -> Result<u32, R::Error> {
        r.try_next()
    }

    pub fn forward<R: DerefMut>(r: &mut R) -> Result<u32, <R::Target as TryRng>::Error>
    where
        R::Target: TryRng,
    {
        take(r)
    }

    pub fn user(s: &mut S) -> Result<u32, ()> {
        let mut r = s;
        take(&mut r)
    }
}

// Higher-ranked where-clauses that hold for every region (see
// `verify/fail/traits/blanket_impl_overlap.rs` for ones that do not): through
// an impl generic in the region, through a blanket impl whose own
// where-clause becomes higher-ranked there, as a projection, and in generic
// code that has the clause itself.
mod higher_ranked {
    use prusti_contracts::*;

    pub trait Tr {}
    impl<'x> Tr for &'x u8 {}
    pub trait Base {}
    impl Base for u16 {}
    pub trait Wrap {}
    impl<'x, T: Base> Wrap for &'x T {}
    impl<X: Wrap> Tr for X {}
    pub trait Pr {
        type A;
    }
    impl<'x> Pr for &'x u8 {
        type A = &'x u8;
    }

    pub struct W<U>(pub U);
    pub struct V<U>(pub U);
    pub trait Get {
        #[pure]
        fn get() -> u32;
    }
    #[refine_trait_spec]
    impl<U> Get for W<U>
    where
        for<'a> &'a U: Tr,
    {
        #[pure]
        #[ensures(result == 2)]
        fn get() -> u32 {
            2
        }
    }
    #[refine_trait_spec]
    impl<U> Get for V<U>
    where
        for<'a> &'a U: Pr<A = &'a u8>,
    {
        #[pure]
        #[ensures(result == 3)]
        fn get() -> u32 {
            3
        }
    }

    #[pure]
    pub fn get<T: Get>() -> u32 {
        T::get()
    }

    pub fn concrete() {
        prusti_assert!(get::<W<u8>>() == 2);
        prusti_assert!(get::<W<u16>>() == 2);
        prusti_assert!(get::<V<u8>>() == 3);
    }

    pub fn generic<T>()
    where
        for<'a> &'a Vec<T>: Tr,
    {
        prusti_assert!(get::<W<Vec<T>>>() == 2);
    }

    // The impls that make these clauses hold have, respectively, an impl
    // parameter determined only by a projection bound, a where-clause to
    // normalize, and a const parameter.
    pub trait Pj {
        type A;
    }
    impl Pj for u8 {
        type A = u16;
    }
    pub trait ByProj {}
    impl<'x, F, R> ByProj for &'x F where F: Pj<A = R> {}
    pub trait Id {
        type Out;
    }
    impl<T> Id for T {
        type Out = T;
    }
    pub trait Normalized {}
    impl<'x, Y> Normalized for &'x Y where for<'b> <&'b Y as Id>::Out: Tr {}
    pub trait Len<const N: usize> {}
    impl<'x, T, const N: usize> Len<N> for &'x T {}

    pub struct X<U>(pub U);
    pub struct Y<U>(pub U);
    pub struct Z<U>(pub U);
    #[refine_trait_spec]
    impl<U> Get for X<U>
    where
        for<'a> &'a U: ByProj,
    {
        #[pure]
        #[ensures(result == 4)]
        fn get() -> u32 {
            4
        }
    }
    #[refine_trait_spec]
    impl<U> Get for Y<U>
    where
        for<'a> &'a U: Normalized,
    {
        #[pure]
        #[ensures(result == 5)]
        fn get() -> u32 {
            5
        }
    }
    #[refine_trait_spec]
    impl<U> Get for Z<U>
    where
        for<'a> &'a U: Len<3>,
    {
        #[pure]
        #[ensures(result == 6)]
        fn get() -> u32 {
            6
        }
    }

    // A clause whose subject is a projection over its bound region is stated
    // only at erased regions (see `HrtbSkeleton::of`), which suffices here.
    pub trait Lend<'a> {
        type Item;
    }
    pub struct Lender;
    impl<'a> Lend<'a> for Lender {
        type Item = &'a u8;
    }
    pub struct L<U>(pub U);
    #[refine_trait_spec]
    impl<U> Get for L<U>
    where
        U: for<'a> Lend<'a>,
        for<'a> <U as Lend<'a>>::Item: Tr,
    {
        #[pure]
        #[ensures(result == 9)]
        fn get() -> u32 {
            9
        }
    }

    pub fn matched() {
        prusti_assert!(get::<X<u8>>() == 4);
        prusti_assert!(get::<Y<u8>>() == 5);
        prusti_assert!(get::<Z<u8>>() == 6);
        prusti_assert!(get::<L<Lender>>() == 9);
    }

    // A higher-ranked clause with a const parameter is encoded.
    pub fn with_const<T, const N: usize>()
    where
        for<'a> &'a T: Len<N>,
    {
    }

    // Distinct types with the same printed path give distinct clauses.
    pub struct Q1<U>(pub U);
    pub struct Q2<U>(pub U);
    const _: () = {
        struct Same<'a, T>(&'a T);
        #[refine_trait_spec]
        impl<U> Get for Q1<U>
        where
            for<'a> Same<'a, U>: Tr,
        {
            #[pure]
            fn get() -> u32 {
                7
            }
        }
    };
    const _: () = {
        struct Same<'a, T>(&'a T);
        #[refine_trait_spec]
        impl<U> Get for Q2<U>
        where
            for<'a> Same<'a, U>: Tr,
        {
            #[pure]
            fn get() -> u32 {
                8
            }
        }
    };
}
