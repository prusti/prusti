//@ compile-flags: -Passert_timeout=45000
pub struct S { pub v: i32 }

pub trait Get { fn get(&mut self) -> &mut i32; }

impl Get for &mut S {
    fn get(&mut self) -> &mut i32 { &mut self.v } //~ ERROR
}
