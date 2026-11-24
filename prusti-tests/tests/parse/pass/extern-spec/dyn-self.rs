use prusti_contracts::*;
use std::any::Any;

// A trait object with several bounds is parenthesized where it replaces
// `Self`, in the receiver (`&(dyn Any + Send)`) and in the specification.
#[extern_spec]
impl dyn Any + Send {
    #[trusted]
    #[pure]
    #[ensures(result == result)]
    fn is<T: Any>(&self) -> bool;

    #[trusted]
    #[ensures(result.is_some() == (self as &Self).is::<T>())]
    fn downcast_ref<T: Any>(&self) -> Option<&T>;
}
