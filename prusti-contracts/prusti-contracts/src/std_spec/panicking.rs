use crate::*;

#[extern_spec(std::rt)]
#[trusted]
#[requires(false)]
#[pure]
fn begin_panic<M: core::any::Any + Send>(msg: M) -> !;

#[extern_spec(std::panic)]
#[trusted]
#[requires(false)]
#[pure]
fn panic_any<M: 'static + core::any::Any + Send>(msg: M) -> !;
