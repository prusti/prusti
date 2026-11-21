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

Is encoded as follows, in a crate named `hello`:

```viper
function f_hello$col$$col$hello(_1s: s_Int_i32): s_Int_i32
{
  (!mir_binop_Lt_Int_i32_Int_i32(_1s, s_Int_i32_cons(3)) ?
    (!mir_binop_Gt_Int_i32_Int_i32(_1s, s_Int_i32_cons(10)) ?
      s_Int_i32_cons(10) :
      (let _0_11s_bb4_s3_i1 ==
        (mir_binop_SubWithOverflow_Int_i32_Int_i32(_1s, s_Int_i32_cons(2))) in
        (!_0_11s_bb4_s3_i1.s_2_Tuple_1.make_concrete_s_Bool ?
          _0_11s_bb4_s3_i1.s_2_Tuple_0.make_concrete_s_Int_i32 :
          s_Int_i32_unreachable()))) :
    (let _0_7s_bb1_s3_i1 ==
      (mir_binop_AddWithOverflow_Int_i32_Int_i32(_1s, s_Int_i32_cons(2))) in
      (!_0_7s_bb1_s3_i1.s_2_Tuple_1.make_concrete_s_Bool ?
        _0_7s_bb1_s3_i1.s_2_Tuple_0.make_concrete_s_Int_i32 :
        s_Int_i32_unreachable())))
}
```

The Viper names contain the path of the function, `hello::hello`, [mangled](mangling.md) with `$col$$col$` for `::`. The names of the `let`-bound variables are those of a debug build of Prusti; a release build names them differently.

The function works on [snapshots](types-snap.md), Prusti's mathematical representation of Rust values (here `s_Int_i32` for `i32`), and each MIR operation becomes a call of a function encoded for that operation (see [`prusti-encoder/src/encoders/builtin/`](https://github.com/prusti/prusti/tree/main/prusti-encoder/src/encoders/builtin)). A checked addition yields a pair of the result and an overflow flag, which are read with the pair's field functions `s_2_Tuple_0` and `s_2_Tuple_1`. A branch that would panic yields `s_Int_i32_unreachable()`, a function without a body whose postcondition is `false`.

The function itself therefore assumes that no panic happens. That this is the case is checked by verifying the body a second time as a Viper method, `m_hello$col$$col$hello`, whose postcondition states that its result equals the result of the function. A possible overflow is reported as an error of this method.

> - [`prusti-encoder/src/encoders/mir_pure.rs` - `MirPureEnc`](https://github.com/prusti/prusti/blob/main/prusti-encoder/src/encoders/mir_pure.rs)
