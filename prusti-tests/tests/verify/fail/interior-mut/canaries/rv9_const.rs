//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;

pub trait P { const A: Real = Real::NONE; }
pub struct S;
impl P for S { const A: Real = Real::WRITE; }

#[ensures(<S as P>::A == Real::NONE)] //~ ERROR
pub fn wrong() {}

#[ensures(<S as P>::A == Real::WRITE)]
pub fn right() {}

pub fn ctrl() { assert!(false); } //~ ERROR
