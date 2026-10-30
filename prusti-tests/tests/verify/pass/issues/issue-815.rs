use prusti_contracts::*;

#[derive(Clone, Copy)]
pub struct A {
    inner: isize,
}

impl A {
    #[pure]
    #[requires(num < isize::MAX as usize)]
    pub fn new(num: usize) -> Self {
        Self {
            inner: num as isize + isize::MIN,
        }
    }

    #[pure]
    pub fn test(&self) {
        let _ = Self::new(2);
    }
}

#[pure]
#[requires(0 <= i && i < 10)]
const fn get(a: &[i32; 10], i: usize) -> i32 {
    a[i]
}

#[requires(0 <= i && i < 10)]
#[ensures(forall(|j: usize|
    (0 <= j && j < 10 && j != old(i)) ==> (get(a, j) == old(get(a, j))),
    triggers=[(get(a, j),)]
))]
#[ensures(get(a, i) == old(v))]
fn set(a: &mut [i32; 10], i: usize, v: i32)  {
    a[i] = v;
}
