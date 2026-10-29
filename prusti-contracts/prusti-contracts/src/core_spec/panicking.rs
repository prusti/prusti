use crate::*;

#[extern_spec(core::panicking)]
#[trusted]
#[requires(false)]
#[pure]
fn panic(expr: &'static str) -> !;

#[extern_spec(core::panicking)]
#[trusted]
#[requires(false)]
#[pure]
fn panic_fmt(fmt: core::fmt::Arguments<'_>) -> !;

#[extern_spec(core::panicking)]
#[trusted]
#[requires(false)]
#[pure]
fn panic_display<T: core::fmt::Display>(x: &T) -> !;

#[extern_spec(core::panicking)]
#[trusted]
#[requires(false)]
#[pure]
fn unreachable_display<T: core::fmt::Display>(x: &T) -> !;

#[extern_spec(core::panicking)]
#[trusted]
#[requires(false)]
#[pure]
fn panic_str_2015(expr: &str) -> !;

#[extern_spec(core::panicking)]
#[trusted]
#[requires(false)]
#[pure]
fn panic_nounwind(expr: &'static str) -> !;

#[extern_spec(core::panicking)]
#[trusted]
#[requires(false)]
#[pure]
pub fn assert_failed<T, U>(
    kind: core::panicking::AssertKind,
    left: &T,
    right: &U,
    args: Option<core::fmt::Arguments<'_>>,
) -> !
where
    T: core::fmt::Debug + ?Sized,
    U: core::fmt::Debug + ?Sized;

#[extern_spec(core::panicking)]
#[trusted]
#[requires(false)]
#[pure]
pub fn assert_matches_failed<T: core::fmt::Debug + ?Sized>(
    left: &T,
    right: &str,
    args: Option<core::fmt::Arguments<'_>>,
) -> !;
