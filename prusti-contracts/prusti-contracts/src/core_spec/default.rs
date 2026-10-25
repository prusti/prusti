use crate::*;

use core::default::Default;

// Not `#[pure]` at the trait level: a default may allocate (e.g. `Box`), and
// two calls of a pure function are equal. The impls below are pure.
#[extern_spec]
trait Default: Sized {
    #[trusted]
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

#[extern_spec]
impl<'a, T> Default for &'a [T] {
    #[trusted]
    #[pure]
    #[ensures(result.len() == 0)]
    fn default() -> &'a [T];
}
