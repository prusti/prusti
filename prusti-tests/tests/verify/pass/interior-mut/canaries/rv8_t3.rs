//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::cell::Cell;
pub struct W<T>(pub T);
pub fn wrap<T>(x: T) -> W<T> { W(x) }
pub fn some<T>(x: T) -> Option<T> { Some(x) }
pub fn first<T>(p: (T, u8)) -> T { p.0 }
pub fn get_ref<T>(w: &W<T>) -> &T { &w.0 }
pub fn uses() -> i32 { let c = Cell::new(1); c.get() }
