# Pure function encoding

To encode specifications and side-effect-free functions (marked with `#[pure]`), Prusti generates Viper [functions](http://viper.ethz.ch/tutorial/?page=1&section=#functions). A Viper function consists of a single expression, so Rust function bodies must be folded accordingly. This is only possible for loop-less functions.

For example, the Rust function:

```rust,noplaypen
# // The next line is only used to enable mdBook doctests for this file to work
# extern crate prusti_contracts;
# 
# use prusti_contracts::*;
# 
#[pure]
fn hello(a: i32) -> i32 {
  let mut b = 10;
  if a < 3 {
    b = a + 2
  } else if a > 10 {
    b = a - 2
  }
  b
}
```

Is encoded as follows, with the [`short_viper_names`](../config/flags.md#short_viper_names) flag and without overflow checks ([`check_overflows`](../config/flags.md#check_overflows) turned off and `-C overflow-checks=off`):

```viper
function f_hello(_1s: s_Int_i32): s_Int_i32
{
  (!mir_binop_Lt_Int_i32_Int_i32(_1s, s_Int_i32_cons(3)) ?
    (!mir_binop_Gt_Int_i32_Int_i32(_1s, s_Int_i32_cons(10)) ?
      s_Int_i32_cons(10) :
      mir_binop_Sub_Int_i32_Int_i32(_1s, s_Int_i32_cons(2))) :
    mir_binop_Add_Int_i32_Int_i32(_1s, s_Int_i32_cons(2)))
}
```

> - [`prusti-encoder/src/encoders/mir_pure/mod.rs` - `MirPureEnc`](https://github.com/prusti/prusti/blob/main/prusti-encoder/src/encoders/mir_pure/mod.rs)
