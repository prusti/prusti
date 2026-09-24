use crate::*;

use core::default::Default;

// TODO: this should ideally be `#[refine_spec(where Self: PureDefault, [pure])]`
// (cf. `PartialEq::eq`), since not every `Default::default` is pure.
#[extern_spec]
trait Default: Sized {
    #[trusted]
    #[pure]
    fn default() -> Self;
}

macro_rules! default_spec {
    ($($t:ty => $v:expr),* $(,)?) => {$(
        #[extern_spec]
        impl Default for $t {
            #[trusted]
            #[pure]
            #[ensures(result == $v)]
            fn default() -> $t;
        }
    )*}
}

default_spec!(
    i8 => 0, i16 => 0, i32 => 0, i64 => 0, i128 => 0, isize => 0,
    u8 => 0, u16 => 0, u32 => 0, u64 => 0, u128 => 0, usize => 0,
    f32 => 0.0, f64 => 0.0,
    bool => false, char => '\0', () => (),
);

#[extern_spec]
impl<T> Default for Option<T> {
    #[trusted]
    #[pure]
    #[ensures(matches!(result, None))]
    fn default() -> Option<T>;
}

/// The default of a tuple is the tuple of the defaults, for every arity
/// `core` implements `Default` for. Built recursively like `core`'s own
/// `tuple_impls!`. The fields are compared with what `===` expands to, as the
/// operator itself does not survive a `macro_rules!` expansion (its three `=`
/// are no longer joint tokens).
macro_rules! impl_default_tuple {
    // Stopping criteria (1-ary tuple)
    ($T:ident) => {
        impl_default_tuple!(@impl $T);
    };
    // Running criteria (n-ary tuple, with n >= 2)
    ($T:ident $( $U:ident )+) => {
        impl_default_tuple!($( $U )+);
        impl_default_tuple!(@impl $T $( $U )+);
    };
    // "Private" internal implementation
    (@impl $( $T:ident )+) => {
        #[extern_spec]
        impl<$($T: Default),+> Default for ($($T,)+) {
            #[trusted]
            #[pure]
            #[ensures($( Ghost::new_ref(&result.${index()}) == Ghost::new_ref(&$T::default()) )&&+)]
            fn default() -> ($($T,)+);
        }
    };
}

impl_default_tuple!(E D C B A Z Y X W V U T);
