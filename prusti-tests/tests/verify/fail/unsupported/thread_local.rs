#![feature(thread_local)]

use prusti_contracts::*;

#[thread_local]
static X: u32 = 0;

// The unsupported read is reported even where the precondition excludes it.
#[requires(!b)]
fn read(b: bool) -> u32 {
    if b { X } else { 0 } //~ERROR: unsupported rvalue
}
