//! Shareable mutable containers.
//!
//! Rust memory safety is based on this rule: Given an object `T`, it is only possible to
//! have one of the following:
//!
//! - Several immutable references (`&T`) to the object (also known as **aliasing**).
//! - One mutable reference (`&mut T`) to the object (also known as **mutability**).
//!
//! This is enforced by the Rust compiler. However, there are situations where this rule is not
//! flexible enough. Sometimes it is required to have multiple references to an object and yet
//! mutate it.
//!
//! Shareable mutable containers exist to permit mutability in a controlled manner, even in the
//! presence of aliasing. [`Cell<T>`], [`RefCell<T>`], and [`OnceCell<T>`] allow doing this in
//! a single-threaded way—they do not implement [`Sync`]. (If you need to do aliasing and
//! mutation among multiple threads, [`Mutex<T>`], [`RwLock<T>`], [`OnceLock<T>`] or [`atomic`]
//! types are the correct data structures to do so).
//!
//! Values of the `Cell<T>`, `RefCell<T>`, and `OnceCell<T>` types may be mutated through shared
//! references (i.e. the common `&T` type), whereas most Rust types can only be mutated through
//! unique (`&mut T`) references. We say these cell types provide 'interior mutability'
//! (mutable via `&T`), in contrast with typical Rust types that exhibit 'inherited mutability'
//! (mutable only via `&mut T`).
//!
//! Cell types come in four flavors: `Cell<T>`, `RefCell<T>`, `OnceCell<T>`, and `LazyCell<T>`.
//! Each provides a different way of providing safe interior mutability.
//!
//! ## `Cell<T>`
//!
//! [`Cell<T>`] implements interior mutability by moving values in and out of the cell. That is, a
//! `&T` to the inner value can never be obtained, and the value itself cannot be directly
//! obtained without replacing it with something else. This type provides the following
//! methods:
//!
//!  - For types that implement [`Copy`], the [`get`](Cell::get) method retrieves the current
//!    interior value by duplicating it.
//!  - For types that implement [`Default`], the [`take`](Cell::take) method replaces the current
//!    interior value with [`Default::default()`] and returns the replaced value.
//!  - All types have:
//!    - [`replace`](Cell::replace): replaces the current interior value and returns the replaced
//!      value.
//!    - [`into_inner`](Cell::into_inner): this method consumes the `Cell<T>` and returns the
//!      interior value.
//!    - [`set`](Cell::set): this method replaces the interior value, dropping the replaced value.
//!
//! `Cell<T>` is typically used for more simple types where copying or moving values isn't too
//! resource intensive (e.g. numbers), and should usually be preferred over other cell types when
//! possible. For larger and non-copy types, `RefCell` provides some advantages.
//!
//! ## `RefCell<T>`
//!
//! [`RefCell<T>`] uses Rust's lifetimes to implement "dynamic borrowing", a process whereby one can
//! claim temporary, exclusive, mutable access to the inner value. Borrows for `RefCell<T>`s are
//! tracked at _runtime_, unlike Rust's native reference types which are entirely tracked
//! statically, at compile time.
//!
//! An immutable reference to a `RefCell`'s inner value (`&T`) can be obtained with
//! [`borrow`](`RefCell::borrow`), and a mutable borrow (`&mut T`) can be obtained with
//! [`borrow_mut`](`RefCell::borrow_mut`). When these functions are called, they first verify that
//! Rust's borrow rules will be satisfied: any number of immutable borrows are allowed or a
//! single mutable borrow is allowed, but never both. If a borrow is attempted that would violate
//! these rules, the thread will panic.
//!
//! The corresponding [`Sync`] version of `RefCell<T>` is [`RwLock<T>`].
//!
//! ## `OnceCell<T>`
//!
//! [`OnceCell<T>`] is somewhat of a hybrid of `Cell` and `RefCell` that works for values that
//! typically only need to be set once. This means that a reference `&T` can be obtained without
//! moving or copying the inner value (unlike `Cell`) but also without runtime checks (unlike
//! `RefCell`). However, once set, its value cannot be updated unless you have a mutable
//! reference to the `OnceCell`.
//!
//! `OnceCell` provides the following methods:
//!
//! - [`get`](OnceCell::get): obtain a reference to the inner value
//! - [`set`](OnceCell::set): set the inner value if it is unset (returns a `Result`)
//! - [`get_or_init`](OnceCell::get_or_init): return the inner value, initializing it if needed
//! - [`get_mut`](OnceCell::get_mut): provide a mutable reference to the inner value, only available
//!   if you have a mutable reference to the cell itself.
//!
//! The corresponding [`Sync`] version of `OnceCell<T>` is [`OnceLock<T>`].
//!
//! ## `LazyCell<T, F>`
//!
//! A common pattern with OnceCell is, for a given OnceCell, to use the same function on every
//! call to [`OnceCell::get_or_init`] with that cell. This is what is offered by [`LazyCell`],
//! which pairs cells of `T` with functions of `F`, and always calls `F` before it yields `&T`.
//! This happens implicitly by simply attempting to dereference the LazyCell to get its contents,
//! so its use is much more transparent with a place which has been initialized by a constant.
//!
//! More complicated patterns that don't fit this description can be built on `OnceCell<T>` instead.
//!
//! `LazyCell` works by providing an implementation of `impl Deref` that calls the function,
//! so you can just use it by dereference (e.g. `*lazy_cell` or `lazy_cell.deref()`).
//!
//! The corresponding [`Sync`] version of `LazyCell<T, F>` is [`LazyLock<T, F>`].
//!
//! # When to choose interior mutability
//!
//! The more common inherited mutability, where one must have unique access to mutate a value, is
//! one of the key language elements that enables Rust to reason strongly about pointer aliasing,
//! statically preventing crash bugs. Because of that, inherited mutability is preferred, and
//! interior mutability is something of a last resort. Since cell types enable mutation where it
//! would otherwise be disallowed though, there are occasions when interior mutability might be
//! appropriate, or even *must* be used, e.g.
//!
//! * Introducing mutability 'inside' of something immutable
//! * Implementation details of logically-immutable methods.
//! * Mutating implementations of [`Clone`].
//!
//! ## Introducing mutability 'inside' of something immutable
//!
//! Many shared smart pointer types, including [`Rc<T>`] and [`Arc<T>`], provide containers that can
//! be cloned and shared between multiple parties. Because the contained values may be
//! multiply-aliased, they can only be borrowed with `&`, not `&mut`. Without cells it would be
//! impossible to mutate data inside of these smart pointers at all.
//!
//! It's very common then to put a `RefCell<T>` inside shared pointer types to reintroduce
//! mutability:
//!
//! ```
//! use std::cell::{RefCell, RefMut};
//! use std::collections::HashMap;
//! use std::rc::Rc;
//!
//! fn main() {
//!     let shared_map: Rc<RefCell<_>> = Rc::new(RefCell::new(HashMap::new()));
//!     // Create a new block to limit the scope of the dynamic borrow
//!     {
//!         let mut map: RefMut<'_, _> = shared_map.borrow_mut();
//!         map.insert("africa", 92388);
//!         map.insert("kyoto", 11837);
//!         map.insert("piccadilly", 11826);
//!         map.insert("marbles", 38);
//!     }
//!
//!     // Note that if we had not let the previous borrow of the cache fall out
//!     // of scope then the subsequent borrow would cause a dynamic thread panic.
//!     // This is the major hazard of using `RefCell`.
//!     let total: i32 = shared_map.borrow().values().sum();
//!     println!("{total}");
//! }
//! ```
//!
//! Note that this example uses `Rc<T>` and not `Arc<T>`. `RefCell<T>`s are for single-threaded
//! scenarios. Consider using [`RwLock<T>`] or [`Mutex<T>`] if you need shared mutability in a
//! multi-threaded situation.
//!
//! ## Implementation details of logically-immutable methods
//!
//! Occasionally it may be desirable not to expose in an API that there is mutation happening
//! "under the hood". This may be because logically the operation is immutable, but e.g., caching
//! forces the implementation to perform mutation; or because you must employ mutation to implement
//! a trait method that was originally defined to take `&self`.
//!
//! ```
//! # #![allow(dead_code)]
//! use std::cell::OnceCell;
//!
//! struct Graph {
//!     edges: Vec<(i32, i32)>,
//!     span_tree_cache: OnceCell<Vec<(i32, i32)>>
//! }
//!
//! impl Graph {
//!     fn minimum_spanning_tree(&self) -> Vec<(i32, i32)> {
//!         self.span_tree_cache
//!             .get_or_init(|| self.calc_span_tree())
//!             .clone()
//!     }
//!
//!     fn calc_span_tree(&self) -> Vec<(i32, i32)> {
//!         // Expensive computation goes here
//!         vec![]
//!     }
//! }
//! ```
//!
//! ## Mutating implementations of `Clone`
//!
//! This is simply a special - but common - case of the previous: hiding mutability for operations
//! that appear to be immutable. The [`clone`](Clone::clone) method is expected to not change the
//! source value, and is declared to take `&self`, not `&mut self`. Therefore, any mutation that
//! happens in the `clone` method must use cell types. For example, [`Rc<T>`] maintains its
//! reference counts within a `Cell<T>`.
//!
//! ```
//! use std::cell::Cell;
//! use std::ptr::NonNull;
//! use std::process::abort;
//! use std::marker::PhantomData;
//!
//! struct Rc<T: ?Sized> {
//!     ptr: NonNull<RcInner<T>>,
//!     phantom: PhantomData<RcInner<T>>,
//! }
//!
//! struct RcInner<T: ?Sized> {
//!     strong: Cell<usize>,
//!     refcount: Cell<usize>,
//!     value: T,
//! }
//!
//! impl<T: ?Sized> Clone for Rc<T> {
//!     fn clone(&self) -> Rc<T> {
//!         self.inc_strong();
//!         Rc {
//!             ptr: self.ptr,
//!             phantom: PhantomData,
//!         }
//!     }
//! }
//!
//! trait RcInnerPtr<T: ?Sized> {
//!
//!     fn inner(&self) -> &RcInner<T>;
//!
//!     fn strong(&self) -> usize {
//!         self.inner().strong.get()
//!     }
//!
//!     fn inc_strong(&self) {
//!         self.inner()
//!             .strong
//!             .set(self.strong()
//!                      .checked_add(1)
//!                      .unwrap_or_else(|| abort() ));
//!     }
//! }
//!
//! impl<T: ?Sized> RcInnerPtr<T> for Rc<T> {
//!    fn inner(&self) -> &RcInner<T> {
//!        unsafe {
//!            self.ptr.as_ref()
//!        }
//!    }
//! }
//! ```
//!
//! [`Arc<T>`]: ../../std/sync/struct.Arc.html
//! [`Rc<T>`]: ../../std/rc/struct.Rc.html
//! [`RwLock<T>`]: ../../std/sync/struct.RwLock.html
//! [`Mutex<T>`]: ../../std/sync/struct.Mutex.html
//! [`OnceLock<T>`]: ../../std/sync/struct.OnceLock.html
//! [`LazyLock<T, F>`]: ../../std/sync/struct.LazyLock.html
//! [`Sync`]: ../../std/marker/trait.Sync.html
//! [`atomic`]: crate::sync::atomic

// std's `debug_refcell` feature does not exist here.
#![allow(unexpected_cfgs)]

use core::cmp::Ordering;
use core::fmt::{self, Debug, Display};
use core::marker::{Destruct, PhantomData};
use core::mem::{self, ManuallyDrop};
use core::ops::{self, Deref, DerefMut};
use core::ptr::{self, NonNull};
use core::range;

pub use core::cell::UnsafeCell;

/// A mutable memory location.
///
/// # Memory layout
///
/// `Cell<T>` has the same [memory layout and caveats as
/// `UnsafeCell<T>`](UnsafeCell#memory-layout). In particular, this means that
/// `Cell<T>` has the same in-memory representation as its inner type `T`.
///
/// # Examples
///
/// In this example, you can see that `Cell<T>` enables mutation inside an
/// immutable struct. In other words, it enables "interior mutability".
///
/// ```
/// use std::cell::Cell;
///
/// struct SomeStruct {
///     regular_field: u8,
///     special_field: Cell<u8>,
/// }
///
/// let my_struct = SomeStruct {
///     regular_field: 0,
///     special_field: Cell::new(1),
/// };
///
/// let new_value = 100;
///
/// // ERROR: `my_struct` is immutable
/// // my_struct.regular_field = new_value;
///
/// // WORKS: although `my_struct` is immutable, `special_field` is a `Cell`,
/// // which can always be mutated
/// my_struct.special_field.set(new_value);
/// assert_eq!(my_struct.special_field.get(), new_value);
/// ```
///
/// See the [module-level documentation](self) for more.
#[repr(transparent)]
pub struct Cell<T: ?Sized> {
    value: UnsafeCell<T>,
}

unsafe impl<T: ?Sized> Send for Cell<T> where T: Send {}

// Note that this negative impl isn't strictly necessary for correctness,
// as `Cell` wraps `UnsafeCell`, which is itself `!Sync`.
// However, given how important `Cell`'s `!Sync`-ness is,
// having an explicit negative impl is nice for documentation purposes
// and results in nicer error messages.
impl<T: ?Sized> !Sync for Cell<T> {}

impl<T: Copy> Clone for Cell<T> {
    #[inline]
    fn clone(&self) -> Cell<T> {
        Cell::new(self.get())
    }
}

const impl<T: [const] Default> Default for Cell<T> {
    /// Creates a `Cell<T>`, with the `Default` value for T.
    #[inline]
    fn default() -> Cell<T> {
        Cell::new(Default::default())
    }
}

impl<T: PartialEq + Copy> PartialEq for Cell<T> {
    #[inline]
    fn eq(&self, other: &Cell<T>) -> bool {
        self.get() == other.get()
    }
}

impl<T: Eq + Copy> Eq for Cell<T> {}

impl<T: PartialOrd + Copy> PartialOrd for Cell<T> {
    #[inline]
    fn partial_cmp(&self, other: &Cell<T>) -> Option<Ordering> {
        self.get().partial_cmp(&other.get())
    }

    #[inline]
    fn lt(&self, other: &Cell<T>) -> bool {
        self.get() < other.get()
    }

    #[inline]
    fn le(&self, other: &Cell<T>) -> bool {
        self.get() <= other.get()
    }

    #[inline]
    fn gt(&self, other: &Cell<T>) -> bool {
        self.get() > other.get()
    }

    #[inline]
    fn ge(&self, other: &Cell<T>) -> bool {
        self.get() >= other.get()
    }
}

impl<T: Ord + Copy> Ord for Cell<T> {
    #[inline]
    fn cmp(&self, other: &Cell<T>) -> Ordering {
        self.get().cmp(&other.get())
    }
}

const impl<T> From<T> for Cell<T> {
    /// Creates a new `Cell<T>` containing the given value.
    fn from(t: T) -> Cell<T> {
        Cell::new(t)
    }
}

impl<T> Cell<T> {
    /// Creates a new `Cell` containing the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    /// ```
    #[inline]
    pub const fn new(value: T) -> Cell<T> {
        Cell { value: UnsafeCell::new(value) }
    }

    /// Sets the contained value.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    ///
    /// c.set(10);
    /// ```
    #[inline]
    pub const fn set(&self, val: T)
    where
        T: [const] Destruct,
    {
        self.replace(val);
    }

    /// Swaps the values of two `Cell`s.
    ///
    /// The difference with `std::mem::swap` is that this function doesn't
    /// require a `&mut` reference.
    ///
    /// # Panics
    ///
    /// This function will panic if `self` and `other` are different `Cell`s that partially overlap.
    /// (Using just standard library methods, it is impossible to create such partially overlapping `Cell`s.
    /// However, unsafe code is allowed to e.g. create two `&Cell<[i32; 2]>` that partially overlap.)
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c1 = Cell::new(5i32);
    /// let c2 = Cell::new(10i32);
    /// c1.swap(&c2);
    /// assert_eq!(10, c1.get());
    /// assert_eq!(5, c2.get());
    /// ```
    #[inline]
    pub fn swap(&self, other: &Self) {
        // This function documents that it *will* panic, and intrinsics::is_nonoverlapping doesn't
        // do the check in const, so trying to use it here would be inviting unnecessary fragility.
        fn is_nonoverlapping<T>(src: *const T, dst: *const T) -> bool {
            let src_usize = src.addr();
            let dst_usize = dst.addr();
            let diff = src_usize.abs_diff(dst_usize);
            diff >= size_of::<T>()
        }

        if ptr::eq(self, other) {
            // Swapping wouldn't change anything.
            return;
        }
        if !is_nonoverlapping(self, other) {
            // See <https://github.com/rust-lang/rust/issues/80778> for why we need to stop here.
            panic!("`Cell::swap` on overlapping non-identical `Cell`s");
        }
        // SAFETY: This can be risky if called from separate threads, but `Cell`
        // is `!Sync` so this won't happen. This also won't invalidate any
        // pointers since `Cell` makes sure nothing else will be pointing into
        // either of these `Cell`s. We also excluded shenanigans like partially overlapping `Cell`s,
        // so `swap` will just properly copy two full values of type `T` back and forth.
        unsafe {
            mem::swap(&mut *self.value.get(), &mut *other.value.get());
        }
    }

    /// Replaces the contained value with `val`, and returns the old contained value.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let cell = Cell::new(5);
    /// assert_eq!(cell.get(), 5);
    /// assert_eq!(cell.replace(10), 5);
    /// assert_eq!(cell.get(), 10);
    /// ```
    #[inline]
    pub const fn replace(&self, val: T) -> T {
        // SAFETY: This can cause data races if called from a separate thread,
        // but `Cell` is `!Sync` so this won't happen.
        mem::replace(unsafe { &mut *self.value.get() }, val)
    }

    /// Unwraps the value, consuming the cell.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    /// let five = c.into_inner();
    ///
    /// assert_eq!(five, 5);
    /// ```
    pub const fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: Copy> Cell<T> {
    /// Returns a copy of the contained value.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    ///
    /// let five = c.get();
    /// ```
    #[inline]
    pub const fn get(&self) -> T {
        // SAFETY: This can cause data races if called from a separate thread,
        // but `Cell` is `!Sync` so this won't happen.
        unsafe { *self.value.get() }
    }

    /// Updates the contained value using a function.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    /// c.update(|x| x + 1);
    /// assert_eq!(c.get(), 6);
    /// ```
    #[inline]
    pub const fn update(&self, f: impl [const] FnOnce(T) -> T)
    where
        // FIXME(const-hack): `Copy` should imply `const Destruct`
        T: [const] Destruct,
    {
        let old = self.get();
        self.set(f(old));
    }
}

impl<T: ?Sized> Cell<T> {
    /// Returns a raw pointer to the underlying data in this cell.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    ///
    /// let ptr = c.as_ptr();
    /// ```
    #[inline]
    pub const fn as_ptr(&self) -> *mut T {
        self.value.get()
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// This call borrows `Cell` mutably (at compile-time) which guarantees
    /// that we possess the only reference.
    ///
    /// However be cautious: this method expects `self` to be mutable, which is
    /// generally not the case when using a `Cell`. If you require interior
    /// mutability by reference, consider using `RefCell` which provides
    /// run-time checked mutable borrows through its [`borrow_mut`] method.
    ///
    /// [`borrow_mut`]: RefCell::borrow_mut()
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let mut c = Cell::new(5);
    /// *c.get_mut() += 1;
    ///
    /// assert_eq!(c.get(), 6);
    /// ```
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    /// Returns a `&Cell<T>` from a `&mut T`
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let slice: &mut [i32] = &mut [1, 2, 3];
    /// let cell_slice: &Cell<[i32]> = Cell::from_mut(slice);
    /// let slice_cell: &[Cell<i32>] = cell_slice.as_slice_of_cells();
    ///
    /// assert_eq!(slice_cell.len(), 3);
    /// ```
    #[inline]
    pub const fn from_mut(t: &mut T) -> &Cell<T> {
        // SAFETY: `&mut` ensures unique access.
        unsafe { &*(t as *mut T as *const Cell<T>) }
    }
}

impl<T: Default> Cell<T> {
    /// Takes the value of the cell, leaving `Default::default()` in its place.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    /// let five = c.take();
    ///
    /// assert_eq!(five, 5);
    /// assert_eq!(c.into_inner(), 0);
    /// ```
    pub const fn take(&self) -> T
    where
        T: [const] Default,
    {
        self.replace(Default::default())
    }
}

impl<T, const N: usize> AsRef<[Cell<T>; N]> for Cell<[T; N]> {
    #[inline]
    fn as_ref(&self) -> &[Cell<T>; N] {
        self.as_array_of_cells()
    }
}

impl<T, const N: usize> AsRef<[Cell<T>]> for Cell<[T; N]> {
    #[inline]
    fn as_ref(&self) -> &[Cell<T>] {
        &*self.as_array_of_cells()
    }
}

impl<T> AsRef<[Cell<T>]> for Cell<[T]> {
    #[inline]
    fn as_ref(&self) -> &[Cell<T>] {
        self.as_slice_of_cells()
    }
}

impl<T> Cell<[T]> {
    /// Returns a `&[Cell<T>]` from a `&Cell<[T]>`
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let slice: &mut [i32] = &mut [1, 2, 3];
    /// let cell_slice: &Cell<[i32]> = Cell::from_mut(slice);
    /// let slice_cell: &[Cell<i32>] = cell_slice.as_slice_of_cells();
    ///
    /// assert_eq!(slice_cell.len(), 3);
    /// ```
    pub const fn as_slice_of_cells(&self) -> &[Cell<T>] {
        // SAFETY: `Cell<T>` has the same memory layout as `T`.
        unsafe { &*(self as *const Cell<[T]> as *const [Cell<T>]) }
    }
}

impl<T, const N: usize> Cell<[T; N]> {
    /// Returns a `&[Cell<T>; N]` from a `&Cell<[T; N]>`
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let mut array: [i32; 3] = [1, 2, 3];
    /// let cell_array: &Cell<[i32; 3]> = Cell::from_mut(&mut array);
    /// let array_cell: &[Cell<i32>; 3] = cell_array.as_array_of_cells();
    /// ```
    pub const fn as_array_of_cells(&self) -> &[Cell<T>; N] {
        // SAFETY: `Cell<T>` has the same memory layout as `T`.
        unsafe { &*(self as *const Cell<[T; N]> as *const [Cell<T>; N]) }
    }
}

/// Types for which cloning `Cell<Self>` is sound.
///
/// # Safety
///
/// Implementing this trait for a type is sound if and only if the following code is sound for T =
/// that type.
///
/// ```
/// #![feature(cell_get_cloned)]
/// # use std::cell::{CloneFromCell, Cell};
/// fn clone_from_cell<T: CloneFromCell>(cell: &Cell<T>) -> T {
///     unsafe { T::clone(&*cell.as_ptr()) }
/// }
/// ```
///
/// Importantly, you can't just implement `CloneFromCell` for any arbitrary `Copy` type, e.g. the
/// following is unsound:
///
/// ```rust
/// # use std::cell::Cell;
///
/// #[derive(Copy, Debug)]
/// pub struct Bad<'a>(Option<&'a Cell<Bad<'a>>>, u8);
///
/// impl Clone for Bad<'_> {
///     fn clone(&self) -> Self {
///         let a: &u8 = &self.1;
///         // when self.0 points to self, we write to self.1 while we have a live `&u8` pointing to
///         // it -- this is UB
///         self.0.unwrap().set(Self(None, 1));
///         dbg!((a, self));
///         Self(None, 0)
///     }
/// }
///
/// // this is not sound
/// // unsafe impl CloneFromCell for Bad<'_> {}
/// ```
// Allow potential overlapping implementations in user code
#[marker]
pub unsafe trait CloneFromCell: Clone {}

// `CloneFromCell` can be implemented for types that don't have indirection and which don't access
// `Cell`s in their `Clone` implementation. A commonly-used subset is covered here.
unsafe impl<T: CloneFromCell, const N: usize> CloneFromCell for [T; N] {}
unsafe impl<T: CloneFromCell> CloneFromCell for Option<T> {}
unsafe impl<T: CloneFromCell, E: CloneFromCell> CloneFromCell for Result<T, E> {}
unsafe impl<T: ?Sized> CloneFromCell for PhantomData<T> {}
unsafe impl<T: CloneFromCell> CloneFromCell for ManuallyDrop<T> {}
unsafe impl<T: CloneFromCell> CloneFromCell for ops::Range<T> {}
unsafe impl<T: CloneFromCell> CloneFromCell for range::Range<T> {}

impl<T: CloneFromCell> Cell<T> {
    /// Get a clone of the `Cell` that contains a copy of the original value.
    ///
    /// This allows a cheaply `Clone`-able type like an `Rc` to be stored in a `Cell`, exposing the
    /// cheaper `clone()` method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(cell_get_cloned)]
    ///
    /// use core::cell::Cell;
    /// use std::rc::Rc;
    ///
    /// let rc = Rc::new(1usize);
    /// let c1 = Cell::new(rc);
    /// let c2 = c1.get_cloned();
    /// assert_eq!(*c2.into_inner(), 1);
    /// ```
    pub fn get_cloned(&self) -> Self {
        // SAFETY: T is CloneFromCell, which guarantees that this is sound.
        Cell::new(T::clone(unsafe { &*self.as_ptr() }))
    }
}

/// A mutable memory location with dynamically checked borrow rules
///
/// See the [module-level documentation](self) for more.
pub struct RefCell<T: ?Sized> {
    borrow: Cell<BorrowCounter>,
    // Stores the location of the earliest currently active borrow.
    // This gets updated whenever we go from having zero borrows
    // to having a single borrow. When a borrow occurs, this gets included
    // in the generated `BorrowError`/`BorrowMutError`
    #[cfg(feature = "debug_refcell")]
    borrowed_at: Cell<Option<&'static core::panic::Location<'static>>>,
    value: UnsafeCell<T>,
}

/// An error returned by [`RefCell::try_borrow`].
#[non_exhaustive]
#[derive(Debug)]
pub struct BorrowError {
    #[cfg(feature = "debug_refcell")]
    location: &'static core::panic::Location<'static>,
}

impl Display for BorrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "debug_refcell")]
        let res = write!(
            f,
            "RefCell already mutably borrowed; a previous borrow was at {}",
            self.location
        );

        #[cfg(not(feature = "debug_refcell"))]
        let res = Display::fmt("RefCell already mutably borrowed", f);

        res
    }
}

/// An error returned by [`RefCell::try_borrow_mut`].
#[non_exhaustive]
#[derive(Debug)]
pub struct BorrowMutError {
    #[cfg(feature = "debug_refcell")]
    location: &'static core::panic::Location<'static>,
}

impl Display for BorrowMutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "debug_refcell")]
        let res = write!(f, "RefCell already borrowed; a previous borrow was at {}", self.location);

        #[cfg(not(feature = "debug_refcell"))]
        let res = Display::fmt("RefCell already borrowed", f);

        res
    }
}

// This ensures the panicking code is outlined from `borrow_mut` for `RefCell`.
#[cfg_attr(not(panic = "immediate-abort"), inline(never))]
#[track_caller]
#[cold]
const fn panic_already_borrowed(_err: BorrowMutError) -> ! {
    panic!("RefCell already borrowed")
}

// This ensures the panicking code is outlined from `borrow` for `RefCell`.
#[cfg_attr(not(panic = "immediate-abort"), inline(never))]
#[track_caller]
#[cold]
const fn panic_already_mutably_borrowed(_err: BorrowError) -> ! {
    panic!("RefCell already mutably borrowed")
}

// Positive values represent the number of `Ref` active. Negative values
// represent the number of `RefMut` active. Multiple `RefMut`s can only be
// active at a time if they refer to distinct, nonoverlapping components of a
// `RefCell` (e.g., different ranges of a slice).
//
// `Ref` and `RefMut` are both two words in size, and so there will likely never
// be enough `Ref`s or `RefMut`s in existence to overflow half of the `usize`
// range. Thus, a `BorrowCounter` will probably never overflow or underflow.
// However, this is not a guarantee, as a pathological program could repeatedly
// create and then mem::forget `Ref`s or `RefMut`s. Thus, all code must
// explicitly check for overflow and underflow in order to avoid unsafety, or at
// least behave correctly in the event that overflow or underflow happens (e.g.,
// see BorrowRef::new).
type BorrowCounter = isize;
const UNUSED: BorrowCounter = 0;

#[inline(always)]
const fn is_writing(x: BorrowCounter) -> bool {
    x < UNUSED
}

#[inline(always)]
const fn is_reading(x: BorrowCounter) -> bool {
    x > UNUSED
}

impl<T> RefCell<T> {
    /// Creates a new `RefCell` containing `value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    /// ```
    #[inline]
    pub const fn new(value: T) -> RefCell<T> {
        RefCell {
            value: UnsafeCell::new(value),
            borrow: Cell::new(UNUSED),
            #[cfg(feature = "debug_refcell")]
            borrowed_at: Cell::new(None),
        }
    }

    /// Consumes the `RefCell`, returning the wrapped value.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// let five = c.into_inner();
    /// ```
    #[inline]
    pub const fn into_inner(self) -> T {
        // Since this function takes `self` (the `RefCell`) by value, the
        // compiler statically verifies that it is not currently borrowed.
        self.value.into_inner()
    }

    /// Replaces the wrapped value with a new one, returning the old value,
    /// without deinitializing either one.
    ///
    /// This function corresponds to [`std::mem::replace`](../mem/fn.replace.html).
    ///
    /// # Panics
    ///
    /// Panics if the value is currently borrowed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    /// let cell = RefCell::new(5);
    /// let old_value = cell.replace(6);
    /// assert_eq!(old_value, 5);
    /// assert_eq!(cell, RefCell::new(6));
    /// ```
    #[inline]
    #[track_caller]
    pub const fn replace(&self, t: T) -> T {
        mem::replace(&mut self.borrow_mut(), t)
    }

    /// Replaces the wrapped value with a new one computed from `f`, returning
    /// the old value, without deinitializing either one.
    ///
    /// # Panics
    ///
    /// Panics if the value is currently borrowed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    /// let cell = RefCell::new(5);
    /// let old_value = cell.replace_with(|&mut old| old + 1);
    /// assert_eq!(old_value, 5);
    /// assert_eq!(cell, RefCell::new(6));
    /// ```
    #[inline]
    #[track_caller]
    pub fn replace_with<F: FnOnce(&mut T) -> T>(&self, f: F) -> T {
        let mut_borrow = &mut *self.borrow_mut();
        let replacement = f(mut_borrow);
        mem::replace(mut_borrow, replacement)
    }

    /// Swaps the wrapped value of `self` with the wrapped value of `other`,
    /// without deinitializing either one.
    ///
    /// This function corresponds to [`std::mem::swap`](../mem/fn.swap.html).
    ///
    /// # Panics
    ///
    /// Panics if the value in either `RefCell` is currently borrowed, or
    /// if `self` and `other` point to the same `RefCell`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    /// let c = RefCell::new(5);
    /// let d = RefCell::new(6);
    /// c.swap(&d);
    /// assert_eq!(c, RefCell::new(6));
    /// assert_eq!(d, RefCell::new(5));
    /// ```
    #[inline]
    pub const fn swap(&self, other: &Self) {
        mem::swap(&mut *self.borrow_mut(), &mut *other.borrow_mut())
    }
}

impl<T: ?Sized> RefCell<T> {
    /// Immutably borrows the wrapped value.
    ///
    /// The borrow lasts until the returned `Ref` exits scope. Multiple
    /// immutable borrows can be taken out at the same time.
    ///
    /// # Panics
    ///
    /// Panics if the value is currently mutably borrowed. For a non-panicking variant, use
    /// [`try_borrow`](#method.try_borrow).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// let borrowed_five = c.borrow();
    /// let borrowed_five2 = c.borrow();
    /// ```
    ///
    /// An example of panic:
    ///
    /// ```should_panic
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// let m = c.borrow_mut();
    /// let b = c.borrow(); // this causes a panic
    /// ```
    #[inline]
    #[track_caller]
    pub const fn borrow(&self) -> Ref<'_, T> {
        match self.try_borrow() {
            Ok(b) => b,
            Err(err) => panic_already_mutably_borrowed(err),
        }
    }

    /// Immutably borrows the wrapped value, returning an error if the value is currently mutably
    /// borrowed.
    ///
    /// The borrow lasts until the returned `Ref` exits scope. Multiple immutable borrows can be
    /// taken out at the same time.
    ///
    /// This is the non-panicking variant of [`borrow`](#method.borrow).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// {
    ///     let m = c.borrow_mut();
    ///     assert!(c.try_borrow().is_err());
    /// }
    ///
    /// {
    ///     let m = c.borrow();
    ///     assert!(c.try_borrow().is_ok());
    /// }
    /// ```
    #[inline]
    #[cfg_attr(feature = "debug_refcell", track_caller)]
    pub const fn try_borrow(&self) -> Result<Ref<'_, T>, BorrowError> {
        match BorrowRef::new(&self.borrow) {
            Some(b) => {
                #[cfg(feature = "debug_refcell")]
                {
                    // `borrowed_at` is always the *first* active borrow
                    if b.borrow.get() == 1 {
                        self.borrowed_at.replace(Some(core::panic::Location::caller()));
                    }
                }

                // SAFETY: `BorrowRef` ensures that there is only immutable access
                // to the value while borrowed.
                let value = unsafe { NonNull::new_unchecked(self.value.get()) };
                Ok(Ref { value, borrow: b })
            }
            None => Err(BorrowError {
                // If a borrow occurred, then we must already have an outstanding borrow,
                // so `borrowed_at` will be `Some`
                #[cfg(feature = "debug_refcell")]
                location: self.borrowed_at.get().unwrap(),
            }),
        }
    }

    /// Mutably borrows the wrapped value.
    ///
    /// The borrow lasts until the returned `RefMut` or all `RefMut`s derived
    /// from it exit scope. The value cannot be borrowed while this borrow is
    /// active.
    ///
    /// # Panics
    ///
    /// Panics if the value is currently borrowed. For a non-panicking variant, use
    /// [`try_borrow_mut`](#method.try_borrow_mut).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new("hello".to_owned());
    ///
    /// *c.borrow_mut() = "bonjour".to_owned();
    ///
    /// assert_eq!(&*c.borrow(), "bonjour");
    /// ```
    ///
    /// An example of panic:
    ///
    /// ```should_panic
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    /// let m = c.borrow();
    ///
    /// let b = c.borrow_mut(); // this causes a panic
    /// ```
    #[inline]
    #[track_caller]
    pub const fn borrow_mut(&self) -> RefMut<'_, T> {
        match self.try_borrow_mut() {
            Ok(b) => b,
            Err(err) => panic_already_borrowed(err),
        }
    }

    /// Mutably borrows the wrapped value, returning an error if the value is currently borrowed.
    ///
    /// The borrow lasts until the returned `RefMut` or all `RefMut`s derived
    /// from it exit scope. The value cannot be borrowed while this borrow is
    /// active.
    ///
    /// This is the non-panicking variant of [`borrow_mut`](#method.borrow_mut).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// {
    ///     let m = c.borrow();
    ///     assert!(c.try_borrow_mut().is_err());
    /// }
    ///
    /// assert!(c.try_borrow_mut().is_ok());
    /// ```
    #[inline]
    #[cfg_attr(feature = "debug_refcell", track_caller)]
    pub const fn try_borrow_mut(&self) -> Result<RefMut<'_, T>, BorrowMutError> {
        match BorrowRefMut::new(&self.borrow) {
            Some(b) => {
                #[cfg(feature = "debug_refcell")]
                {
                    self.borrowed_at.replace(Some(core::panic::Location::caller()));
                }

                // SAFETY: `BorrowRefMut` guarantees unique access.
                let value = unsafe { NonNull::new_unchecked(self.value.get()) };
                Ok(RefMut { value, borrow: b, marker: PhantomData })
            }
            None => Err(BorrowMutError {
                // If a borrow occurred, then we must already have an outstanding borrow,
                // so `borrowed_at` will be `Some`
                #[cfg(feature = "debug_refcell")]
                location: self.borrowed_at.get().unwrap(),
            }),
        }
    }

    /// Returns a raw pointer to the underlying data in this cell.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// let ptr = c.as_ptr();
    /// ```
    #[inline]
    pub const fn as_ptr(&self) -> *mut T {
        self.value.get()
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this method borrows `RefCell` mutably, it is statically guaranteed
    /// that no borrows to the underlying data exist. The dynamic checks inherent
    /// in [`borrow_mut`] and most other methods of `RefCell` are therefore
    /// unnecessary. Note that this method does not reset the borrowing state if borrows were previously leaked
    /// (e.g., via [`forget()`] on a [`Ref`] or [`RefMut`]). For that purpose,
    /// consider using the unstable [`undo_leak`] method.
    ///
    /// This method can only be called if `RefCell` can be mutably borrowed,
    /// which in general is only the case directly after the `RefCell` has
    /// been created. In these situations, skipping the aforementioned dynamic
    /// borrowing checks may yield better ergonomics and runtime-performance.
    ///
    /// In most situations where `RefCell` is used, it can't be borrowed mutably.
    /// Use [`borrow_mut`] to get mutable access to the underlying data then.
    ///
    /// [`borrow_mut`]: RefCell::borrow_mut()
    /// [`forget()`]: mem::forget
    /// [`undo_leak`]: RefCell::undo_leak()
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let mut c = RefCell::new(5);
    /// *c.get_mut() += 1;
    ///
    /// assert_eq!(c, RefCell::new(6));
    /// ```
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    /// Undo the effect of leaked guards on the borrow state of the `RefCell`.
    ///
    /// This call is similar to [`get_mut`] but more specialized. It borrows `RefCell` mutably to
    /// ensure no borrows exist and then resets the state tracking shared borrows. This is relevant
    /// if some `Ref` or `RefMut` borrows have been leaked.
    ///
    /// [`get_mut`]: RefCell::get_mut()
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(cell_leak)]
    /// use std::cell::RefCell;
    ///
    /// let mut c = RefCell::new(0);
    /// std::mem::forget(c.borrow_mut());
    ///
    /// assert!(c.try_borrow().is_err());
    /// c.undo_leak();
    /// assert!(c.try_borrow().is_ok());
    /// ```
    pub const fn undo_leak(&mut self) -> &mut T {
        *self.borrow.get_mut() = UNUSED;
        self.get_mut()
    }

    /// Immutably borrows the wrapped value, returning an error if the value is
    /// currently mutably borrowed.
    ///
    /// # Safety
    ///
    /// Unlike `RefCell::borrow`, this method is unsafe because it does not
    /// return a `Ref`, thus leaving the borrow flag untouched. Mutably
    /// borrowing the `RefCell` while the reference returned by this method
    /// is alive is undefined behavior.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// {
    ///     let m = c.borrow_mut();
    ///     assert!(unsafe { c.try_borrow_unguarded() }.is_err());
    /// }
    ///
    /// {
    ///     let m = c.borrow();
    ///     assert!(unsafe { c.try_borrow_unguarded() }.is_ok());
    /// }
    /// ```
    #[inline]
    pub const unsafe fn try_borrow_unguarded(&self) -> Result<&T, BorrowError> {
        if !is_writing(self.borrow.get()) {
            // SAFETY: We check that nobody is actively writing now, but it is
            // the caller's responsibility to ensure that nobody writes until
            // the returned reference is no longer in use.
            // Also, `self.value.get()` refers to the value owned by `self`
            // and is thus guaranteed to be valid for the lifetime of `self`.
            Ok(unsafe { &*self.value.get() })
        } else {
            Err(BorrowError {
                // If a borrow occurred, then we must already have an outstanding borrow,
                // so `borrowed_at` will be `Some`
                #[cfg(feature = "debug_refcell")]
                location: self.borrowed_at.get().unwrap(),
            })
        }
    }
}

impl<T: Default> RefCell<T> {
    /// Takes the wrapped value, leaving `Default::default()` in its place.
    ///
    /// # Panics
    ///
    /// Panics if the value is currently borrowed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    /// let five = c.take();
    ///
    /// assert_eq!(five, 5);
    /// assert_eq!(c.into_inner(), 0);
    /// ```
    pub fn take(&self) -> T {
        self.replace(Default::default())
    }
}

unsafe impl<T: ?Sized> Send for RefCell<T> where T: Send {}

impl<T: ?Sized> !Sync for RefCell<T> {}

impl<T: Clone> Clone for RefCell<T> {
    /// # Panics
    ///
    /// Panics if the value is currently mutably borrowed.
    #[inline]
    #[track_caller]
    fn clone(&self) -> RefCell<T> {
        RefCell::new(self.borrow().clone())
    }

    /// # Panics
    ///
    /// Panics if `source` is currently mutably borrowed.
    #[inline]
    #[track_caller]
    fn clone_from(&mut self, source: &Self) {
        self.get_mut().clone_from(&source.borrow())
    }
}

const impl<T: [const] Default> Default for RefCell<T> {
    /// Creates a `RefCell<T>`, with the `Default` value for T.
    #[inline]
    fn default() -> RefCell<T> {
        RefCell::new(Default::default())
    }
}

impl<T: ?Sized + PartialEq> PartialEq for RefCell<T> {
    /// # Panics
    ///
    /// Panics if the value in either `RefCell` is currently mutably borrowed.
    #[inline]
    fn eq(&self, other: &RefCell<T>) -> bool {
        *self.borrow() == *other.borrow()
    }
}

impl<T: ?Sized + Eq> Eq for RefCell<T> {}

impl<T: ?Sized + PartialOrd> PartialOrd for RefCell<T> {
    /// # Panics
    ///
    /// Panics if the value in either `RefCell` is currently mutably borrowed.
    #[inline]
    fn partial_cmp(&self, other: &RefCell<T>) -> Option<Ordering> {
        self.borrow().partial_cmp(&*other.borrow())
    }

    /// # Panics
    ///
    /// Panics if the value in either `RefCell` is currently mutably borrowed.
    #[inline]
    fn lt(&self, other: &RefCell<T>) -> bool {
        *self.borrow() < *other.borrow()
    }

    /// # Panics
    ///
    /// Panics if the value in either `RefCell` is currently mutably borrowed.
    #[inline]
    fn le(&self, other: &RefCell<T>) -> bool {
        *self.borrow() <= *other.borrow()
    }

    /// # Panics
    ///
    /// Panics if the value in either `RefCell` is currently mutably borrowed.
    #[inline]
    fn gt(&self, other: &RefCell<T>) -> bool {
        *self.borrow() > *other.borrow()
    }

    /// # Panics
    ///
    /// Panics if the value in either `RefCell` is currently mutably borrowed.
    #[inline]
    fn ge(&self, other: &RefCell<T>) -> bool {
        *self.borrow() >= *other.borrow()
    }
}

impl<T: ?Sized + Ord> Ord for RefCell<T> {
    /// # Panics
    ///
    /// Panics if the value in either `RefCell` is currently mutably borrowed.
    #[inline]
    fn cmp(&self, other: &RefCell<T>) -> Ordering {
        self.borrow().cmp(&*other.borrow())
    }
}

const impl<T> From<T> for RefCell<T> {
    /// Creates a new `RefCell<T>` containing the given value.
    fn from(t: T) -> RefCell<T> {
        RefCell::new(t)
    }
}

struct BorrowRef<'b> {
    borrow: &'b Cell<BorrowCounter>,
}

impl<'b> BorrowRef<'b> {
    #[inline]
    const fn new(borrow: &'b Cell<BorrowCounter>) -> Option<BorrowRef<'b>> {
        let b = borrow.get().wrapping_add(1);
        if !is_reading(b) {
            // Incrementing borrow can result in a non-reading value (<= 0) in these cases:
            // 1. It was < 0, i.e. there are writing borrows, so we can't allow a read borrow
            //    due to Rust's reference aliasing rules
            // 2. It was isize::MAX (the max amount of reading borrows) and it overflowed
            //    into isize::MIN (the max amount of writing borrows) so we can't allow
            //    an additional read borrow because isize can't represent so many read borrows
            //    (this can only happen if you mem::forget more than a small constant amount of
            //    `Ref`s, which is not good practice)
            None
        } else {
            // Incrementing borrow can result in a reading value (> 0) in these cases:
            // 1. It was = 0, i.e. it wasn't borrowed, and we are taking the first read borrow
            // 2. It was > 0 and < isize::MAX, i.e. there were read borrows, and isize
            //    is large enough to represent having one more read borrow
            borrow.replace(b);
            Some(BorrowRef { borrow })
        }
    }
}

const impl Drop for BorrowRef<'_> {
    #[inline]
    fn drop(&mut self) {
        let borrow = self.borrow.get();
        debug_assert!(is_reading(borrow));
        self.borrow.replace(borrow - 1);
    }
}

const impl Clone for BorrowRef<'_> {
    #[inline]
    fn clone(&self) -> Self {
        // Since this Ref exists, we know the borrow flag
        // is a reading borrow.
        let borrow = self.borrow.get();
        debug_assert!(is_reading(borrow));
        // Prevent the borrow counter from overflowing into
        // a writing borrow.
        assert!(borrow != BorrowCounter::MAX);
        self.borrow.replace(borrow + 1);
        BorrowRef { borrow: self.borrow }
    }
}

/// Wraps a borrowed reference to a value in a `RefCell` box.
/// A wrapper type for an immutably borrowed value from a `RefCell<T>`.
///
/// See the [module-level documentation](self) for more.
#[must_not_suspend = "holding a Ref across suspend points can cause BorrowErrors"]
pub struct Ref<'b, T: ?Sized + 'b> {
    // NB: we use a pointer instead of `&'b T` to avoid `noalias` violations, because a
    // `Ref` argument doesn't hold immutability for its whole scope, only until it drops.
    // `NonNull` is also covariant over `T`, just like we would have with `&T`.
    value: NonNull<T>,
    borrow: BorrowRef<'b>,
}

const impl<T: ?Sized> Deref for Ref<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // SAFETY: the value is accessible as long as we hold our borrow.
        unsafe { self.value.as_ref() }
    }
}

impl<'b, T: ?Sized> Ref<'b, T> {
    /// Copies a `Ref`.
    ///
    /// The `RefCell` is already immutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `Ref::clone(...)`. A `Clone` implementation or a method would interfere
    /// with the widespread use of `r.borrow().clone()` to clone the contents of
    /// a `RefCell`.
    #[must_use]
    #[inline]
    pub const fn clone(orig: &Ref<'b, T>) -> Ref<'b, T> {
        Ref { value: orig.value, borrow: orig.borrow.clone() }
    }

    /// Makes a new `Ref` for a component of the borrowed data.
    ///
    /// The `RefCell` is already immutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as `Ref::map(...)`.
    /// A method would interfere with methods of the same name on the contents
    /// of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::{RefCell, Ref};
    ///
    /// let c = RefCell::new((5, 'b'));
    /// let b1: Ref<'_, (u32, char)> = c.borrow();
    /// let b2: Ref<'_, u32> = Ref::map(b1, |t| &t.0);
    /// assert_eq!(*b2, 5)
    /// ```
    #[inline]
    pub fn map<U: ?Sized, F>(orig: Ref<'b, T>, f: F) -> Ref<'b, U>
    where
        F: FnOnce(&T) -> &U,
    {
        Ref { value: NonNull::from(f(&*orig)), borrow: orig.borrow }
    }

    /// Makes a new `Ref` for an optional component of the borrowed data. The
    /// original guard is returned as an `Err(..)` if the closure returns
    /// `None`.
    ///
    /// The `RefCell` is already immutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `Ref::filter_map(...)`. A method would interfere with methods of the same
    /// name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::{RefCell, Ref};
    ///
    /// let c = RefCell::new(vec![1, 2, 3]);
    /// let b1: Ref<'_, Vec<u32>> = c.borrow();
    /// let b2: Result<Ref<'_, u32>, _> = Ref::filter_map(b1, |v| v.get(1));
    /// assert_eq!(*b2.unwrap(), 2);
    /// ```
    #[inline]
    pub fn filter_map<U: ?Sized, F>(orig: Ref<'b, T>, f: F) -> Result<Ref<'b, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
    {
        match f(&*orig) {
            Some(value) => Ok(Ref { value: NonNull::from(value), borrow: orig.borrow }),
            None => Err(orig),
        }
    }

    /// Tries to makes a new `Ref` for a component of the borrowed data.
    /// On failure, the original guard is returned alongside with the error
    /// returned by the closure.
    ///
    /// The `RefCell` is already immutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `Ref::try_map(...)`. A method would interfere with methods of the same
    /// name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(refcell_try_map)]
    /// use std::cell::{RefCell, Ref};
    /// use std::str::{from_utf8, Utf8Error};
    ///
    /// let c = RefCell::new(vec![0xF0, 0x9F, 0xA6 ,0x80]);
    /// let b1: Ref<'_, Vec<u8>> = c.borrow();
    /// let b2: Result<Ref<'_, str>, _> = Ref::try_map(b1, |v| from_utf8(v));
    /// assert_eq!(&*b2.unwrap(), "🦀");
    ///
    /// let c = RefCell::new(vec![0xF0, 0x9F, 0xA6]);
    /// let b1: Ref<'_, Vec<u8>> = c.borrow();
    /// let b2: Result<_, (Ref<'_, Vec<u8>>, Utf8Error)> = Ref::try_map(b1, |v| from_utf8(v));
    /// let (b3, e) = b2.unwrap_err();
    /// assert_eq!(*b3, vec![0xF0, 0x9F, 0xA6]);
    /// assert_eq!(e.valid_up_to(), 0);
    /// ```
    #[inline]
    pub fn try_map<U: ?Sized, E>(
        orig: Ref<'b, T>,
        f: impl FnOnce(&T) -> Result<&U, E>,
    ) -> Result<Ref<'b, U>, (Self, E)> {
        match f(&*orig) {
            Ok(value) => Ok(Ref { value: NonNull::from(value), borrow: orig.borrow }),
            Err(e) => Err((orig, e)),
        }
    }

    /// Splits a `Ref` into multiple `Ref`s for different components of the
    /// borrowed data.
    ///
    /// The `RefCell` is already immutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `Ref::map_split(...)`. A method would interfere with methods of the same
    /// name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::{Ref, RefCell};
    ///
    /// let cell = RefCell::new([1, 2, 3, 4]);
    /// let borrow = cell.borrow();
    /// let (begin, end) = Ref::map_split(borrow, |slice| slice.split_at(2));
    /// assert_eq!(*begin, [1, 2]);
    /// assert_eq!(*end, [3, 4]);
    /// ```
    #[inline]
    pub fn map_split<U: ?Sized, V: ?Sized, F>(orig: Ref<'b, T>, f: F) -> (Ref<'b, U>, Ref<'b, V>)
    where
        F: FnOnce(&T) -> (&U, &V),
    {
        let (a, b) = f(&*orig);
        let borrow = orig.borrow.clone();
        (
            Ref { value: NonNull::from(a), borrow },
            Ref { value: NonNull::from(b), borrow: orig.borrow },
        )
    }

    /// Converts into a reference to the underlying data.
    ///
    /// The underlying `RefCell` can never be mutably borrowed from again and will always appear
    /// already immutably borrowed. It is not a good idea to leak more than a constant number of
    /// references. The `RefCell` can be immutably borrowed again if only a smaller number of leaks
    /// have occurred in total.
    ///
    /// This is an associated function that needs to be used as
    /// `Ref::leak(...)`. A method would interfere with methods of the
    /// same name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(cell_leak)]
    /// use std::cell::{RefCell, Ref};
    /// let cell = RefCell::new(0);
    ///
    /// let value = Ref::leak(cell.borrow());
    /// assert_eq!(*value, 0);
    ///
    /// assert!(cell.try_borrow().is_ok());
    /// assert!(cell.try_borrow_mut().is_err());
    /// ```
    pub const fn leak(orig: Ref<'b, T>) -> &'b T {
        // By forgetting this Ref we ensure that the borrow counter in the RefCell can't go back to
        // UNUSED within the lifetime `'b`. Resetting the reference tracking state would require a
        // unique reference to the borrowed RefCell. No further mutable references can be created
        // from the original cell.
        mem::forget(orig.borrow);
        // SAFETY: after forgetting, we can form a reference for the rest of lifetime `'b`.
        unsafe { orig.value.as_ref() }
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for Ref<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<'b, T: ?Sized> RefMut<'b, T> {
    /// Makes a new `RefMut` for a component of the borrowed data, e.g., an enum
    /// variant.
    ///
    /// The `RefCell` is already mutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `RefMut::map(...)`. A method would interfere with methods of the same
    /// name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::{RefCell, RefMut};
    ///
    /// let c = RefCell::new((5, 'b'));
    /// {
    ///     let b1: RefMut<'_, (u32, char)> = c.borrow_mut();
    ///     let mut b2: RefMut<'_, u32> = RefMut::map(b1, |t| &mut t.0);
    ///     assert_eq!(*b2, 5);
    ///     *b2 = 42;
    /// }
    /// assert_eq!(*c.borrow(), (42, 'b'));
    /// ```
    #[inline]
    pub fn map<U: ?Sized, F>(mut orig: RefMut<'b, T>, f: F) -> RefMut<'b, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        let value = NonNull::from(f(&mut *orig));
        RefMut { value, borrow: orig.borrow, marker: PhantomData }
    }

    /// Makes a new `RefMut` for an optional component of the borrowed data. The
    /// original guard is returned as an `Err(..)` if the closure returns
    /// `None`.
    ///
    /// The `RefCell` is already mutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `RefMut::filter_map(...)`. A method would interfere with methods of the
    /// same name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::{RefCell, RefMut};
    ///
    /// let c = RefCell::new(vec![1, 2, 3]);
    ///
    /// {
    ///     let b1: RefMut<'_, Vec<u32>> = c.borrow_mut();
    ///     let mut b2: Result<RefMut<'_, u32>, _> = RefMut::filter_map(b1, |v| v.get_mut(1));
    ///
    ///     if let Ok(mut b2) = b2 {
    ///         *b2 += 2;
    ///     }
    /// }
    ///
    /// assert_eq!(*c.borrow(), vec![1, 4, 3]);
    /// ```
    #[inline]
    pub fn filter_map<U: ?Sized, F>(mut orig: RefMut<'b, T>, f: F) -> Result<RefMut<'b, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
    {
        // SAFETY: function holds onto an exclusive reference for the duration
        // of its call through `orig`, and the pointer is only de-referenced
        // inside of the function call never allowing the exclusive reference to
        // escape.
        match f(&mut *orig) {
            Some(value) => {
                Ok(RefMut { value: NonNull::from(value), borrow: orig.borrow, marker: PhantomData })
            }
            None => Err(orig),
        }
    }

    /// Tries to makes a new `RefMut` for a component of the borrowed data.
    /// On failure, the original guard is returned alongside with the error
    /// returned by the closure.
    ///
    /// The `RefCell` is already mutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `RefMut::try_map(...)`. A method would interfere with methods of the same
    /// name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(refcell_try_map)]
    /// use std::cell::{RefCell, RefMut};
    /// use std::str::{from_utf8_mut, Utf8Error};
    ///
    /// let c = RefCell::new(vec![0x68, 0x65, 0x6C, 0x6C, 0x6F]);
    /// {
    ///     let b1: RefMut<'_, Vec<u8>> = c.borrow_mut();
    ///     let b2: Result<RefMut<'_, str>, _> = RefMut::try_map(b1, |v| from_utf8_mut(v));
    ///     let mut b2 = b2.unwrap();
    ///     assert_eq!(&*b2, "hello");
    ///     b2.make_ascii_uppercase();
    /// }
    /// assert_eq!(*c.borrow(), "HELLO".as_bytes());
    ///
    /// let c = RefCell::new(vec![0xFF]);
    /// let b1: RefMut<'_, Vec<u8>> = c.borrow_mut();
    /// let b2: Result<_, (RefMut<'_, Vec<u8>>, Utf8Error)> = RefMut::try_map(b1, |v| from_utf8_mut(v));
    /// let (b3, e) = b2.unwrap_err();
    /// assert_eq!(*b3, vec![0xFF]);
    /// assert_eq!(e.valid_up_to(), 0);
    /// ```
    #[inline]
    pub fn try_map<U: ?Sized, E>(
        mut orig: RefMut<'b, T>,
        f: impl FnOnce(&mut T) -> Result<&mut U, E>,
    ) -> Result<RefMut<'b, U>, (Self, E)> {
        // SAFETY: function holds onto an exclusive reference for the duration
        // of its call through `orig`, and the pointer is only de-referenced
        // inside of the function call never allowing the exclusive reference to
        // escape.
        match f(&mut *orig) {
            Ok(value) => {
                Ok(RefMut { value: NonNull::from(value), borrow: orig.borrow, marker: PhantomData })
            }
            Err(e) => Err((orig, e)),
        }
    }

    /// Splits a `RefMut` into multiple `RefMut`s for different components of the
    /// borrowed data.
    ///
    /// The underlying `RefCell` will remain mutably borrowed until both
    /// returned `RefMut`s go out of scope.
    ///
    /// The `RefCell` is already mutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `RefMut::map_split(...)`. A method would interfere with methods of the
    /// same name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::{RefCell, RefMut};
    ///
    /// let cell = RefCell::new([1, 2, 3, 4]);
    /// let borrow = cell.borrow_mut();
    /// let (mut begin, mut end) = RefMut::map_split(borrow, |slice| slice.split_at_mut(2));
    /// assert_eq!(*begin, [1, 2]);
    /// assert_eq!(*end, [3, 4]);
    /// begin.copy_from_slice(&[4, 3]);
    /// end.copy_from_slice(&[2, 1]);
    /// ```
    #[inline]
    pub fn map_split<U: ?Sized, V: ?Sized, F>(
        mut orig: RefMut<'b, T>,
        f: F,
    ) -> (RefMut<'b, U>, RefMut<'b, V>)
    where
        F: FnOnce(&mut T) -> (&mut U, &mut V),
    {
        let borrow = orig.borrow.clone();
        let (a, b) = f(&mut *orig);
        (
            RefMut { value: NonNull::from(a), borrow, marker: PhantomData },
            RefMut { value: NonNull::from(b), borrow: orig.borrow, marker: PhantomData },
        )
    }

    /// Converts into a mutable reference to the underlying data.
    ///
    /// The underlying `RefCell` can not be borrowed from again and will always appear already
    /// mutably borrowed, making the returned reference the only to the interior.
    ///
    /// This is an associated function that needs to be used as
    /// `RefMut::leak(...)`. A method would interfere with methods of the
    /// same name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(cell_leak)]
    /// use std::cell::{RefCell, RefMut};
    /// let cell = RefCell::new(0);
    ///
    /// let value = RefMut::leak(cell.borrow_mut());
    /// assert_eq!(*value, 0);
    /// *value = 1;
    ///
    /// assert!(cell.try_borrow_mut().is_err());
    /// ```
    pub const fn leak(mut orig: RefMut<'b, T>) -> &'b mut T {
        // By forgetting this BorrowRefMut we ensure that the borrow counter in the RefCell can't
        // go back to UNUSED within the lifetime `'b`. Resetting the reference tracking state would
        // require a unique reference to the borrowed RefCell. No further references can be created
        // from the original cell within that lifetime, making the current borrow the only
        // reference for the remaining lifetime.
        mem::forget(orig.borrow);
        // SAFETY: after forgetting, we can form a reference for the rest of lifetime `'b`.
        unsafe { orig.value.as_mut() }
    }
}

struct BorrowRefMut<'b> {
    borrow: &'b Cell<BorrowCounter>,
}

const impl Drop for BorrowRefMut<'_> {
    #[inline]
    fn drop(&mut self) {
        let borrow = self.borrow.get();
        debug_assert!(is_writing(borrow));
        self.borrow.replace(borrow + 1);
    }
}

impl<'b> BorrowRefMut<'b> {
    #[inline]
    const fn new(borrow: &'b Cell<BorrowCounter>) -> Option<BorrowRefMut<'b>> {
        // NOTE: Unlike BorrowRefMut::clone, new is called to create the initial
        // mutable reference, and so there must currently be no existing
        // references. Thus, while clone increments the mutable refcount, here
        // we explicitly only allow going from UNUSED to UNUSED - 1.
        match borrow.get() {
            UNUSED => {
                borrow.replace(UNUSED - 1);
                Some(BorrowRefMut { borrow })
            }
            _ => None,
        }
    }

    // Clones a `BorrowRefMut`.
    //
    // This is only valid if each `BorrowRefMut` is used to track a mutable
    // reference to a distinct, nonoverlapping range of the original object.
    // This isn't in a Clone impl so that code doesn't call this implicitly.
    #[inline]
    fn clone(&self) -> BorrowRefMut<'b> {
        let borrow = self.borrow.get();
        debug_assert!(is_writing(borrow));
        // Prevent the borrow counter from underflowing.
        assert!(borrow != BorrowCounter::MIN);
        self.borrow.set(borrow - 1);
        BorrowRefMut { borrow: self.borrow }
    }
}

/// A wrapper type for a mutably borrowed value from a `RefCell<T>`.
///
/// See the [module-level documentation](self) for more.
#[must_not_suspend = "holding a RefMut across suspend points can cause BorrowErrors"]
pub struct RefMut<'b, T: ?Sized + 'b> {
    // NB: we use a pointer instead of `&'b mut T` to avoid `noalias` violations, because a
    // `RefMut` argument doesn't hold exclusivity for its whole scope, only until it drops.
    value: NonNull<T>,
    borrow: BorrowRefMut<'b>,
    // `NonNull` is covariant over `T`, so we need to reintroduce invariance.
    marker: PhantomData<&'b mut T>,
}

const impl<T: ?Sized> Deref for RefMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // SAFETY: the value is accessible as long as we hold our borrow.
        unsafe { self.value.as_ref() }
    }
}

const impl<T: ?Sized> DerefMut for RefMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the value is accessible as long as we hold our borrow.
        unsafe { self.value.as_mut() }
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RefMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}
