// Two-phase borrows: the borrowed place stays readable until the borrow is
// activated, and the call sees its value.
use prusti_contracts::*;

struct V {
    n: usize,
    m: usize,
}

impl V {
    #[pure]
    fn len(&self) -> usize {
        self.n
    }

    #[ensures(result == self.n)]
    fn get_n(&self) -> usize {
        self.n
    }

    #[ensures(self.m == x && self.n == old(self.n))]
    fn set(&mut self, x: usize) {
        self.m = x;
    }
}

struct W {
    v: V,
    k: usize,
}

#[ensures(result.m == old(v.n))]
fn pure_read(mut v: V) -> V {
    v.set(v.len());
    v
}

#[ensures(result.m == old(v.n))]
fn shared_reborrow(mut v: V) -> V {
    v.set(v.get_n());
    v
}

#[ensures(v.m == old(v.n))]
fn through_mut_ref(v: &mut V) {
    v.set(v.get_n());
}

#[ensures(result.m == old(v.n))]
fn field_read(mut v: V) -> V {
    v.set(v.n);
    v
}

#[ensures(w.v.m == old(w.k))]
fn sibling_read(w: &mut W) {
    w.v.set(w.k);
}

#[ensures(result.v.m == old(w.k))]
fn under_shared_borrow(mut w: W) -> W {
    let r = &w;
    w.v.set(r.k);
    w
}

#[requires(*i < 100)]
#[ensures(*i == old(*i) + 1)]
fn inc(i: &mut usize) {
    *i += 1;
}

#[requires(*x < 100)]
#[ensures(*x == old(*x) + 1)]
fn reborrow_argument(x: &mut usize) {
    inc(x);
}

// A borrow of the referent of a shared reference field outlives mutable
// borrows of the struct holding the reference.
struct Table {
    size: usize,
}

struct Cache<'a> {
    table: &'a Table,
    hits: usize,
}

impl<'a> Cache<'a> {
    #[ensures(self.hits == x)]
    fn record(&mut self, x: usize) {
        self.hits = x;
    }
}

#[ensures(result == old(cache.table.size) && cache.hits == old(cache.table.size))]
fn activation_behind_shared_ref(cache: &mut Cache) -> usize {
    let size = &cache.table.size;
    cache.record(*size);
    *size
}

#[ensures(result == old(cache.table.size) && cache.hits == 0)]
fn reborrow_behind_shared_ref(cache: &mut Cache) -> usize {
    let size = &cache.table.size;
    let r = &mut *cache;
    r.hits = 0;
    *size
}
