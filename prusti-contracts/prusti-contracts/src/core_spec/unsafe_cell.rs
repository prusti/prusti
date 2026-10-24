use crate::{cell::im_get, *};

#[extern_spec]
impl<T: ?Sized> core::cell::UnsafeCell<T> {
    /// The pointer to the contents (an interior location: the permission to
    /// them is not the cell's, see `prusti_contracts::cell`).
    #[trusted]
    #[ensures(result as *const T === im_get(self) as *const T)]
    pub const fn get(&self) -> *mut T;
}
