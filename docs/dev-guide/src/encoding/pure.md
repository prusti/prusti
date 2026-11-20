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

Is encoded as:

```viper
function hello(a: Int): Int
  requires true
{
  !(a < 3) ? (!(a > 10) ? 10 : a - 2) : a + 2
}
```

> - [`prusti-encoder/src/encoders/mir_pure.rs` - `MirPureEnc`](https://github.com/prusti/prusti/blob/main/prusti-encoder/src/encoders/mir_pure.rs)
