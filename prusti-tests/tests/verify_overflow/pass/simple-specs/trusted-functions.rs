use prusti_contracts::*;

struct ThreadRng;

fn thread_rng() -> ThreadRng {
    ThreadRng
}

struct RandWrapper {
    rng: ThreadRng
}

impl ThreadRng {
    #[trusted]
    fn gen_range(&self, min: u32, max: u32) -> u32 {
        unimplemented!();
    }
}

impl RandWrapper {
    #[trusted]
    #[ensures(result >= min && result < max)]
    pub fn gen_range(&mut self, min: u32, max: u32) -> u32 {
        self.rng.gen_range(min, max)
    }
}

#[ensures(result >= 0 && result < 10)]
fn func() -> u32{
    let mut rw = RandWrapper { rng: thread_rng() };
    rw.gen_range(0, 10)
}

pub struct VecWrapperI32{
    v: Vec<i32>
}

impl VecWrapperI32 {
    // Encoded as body-less Viper function
    #[trusted]
    #[pure]
    pub fn len(&self) -> usize {
        self.v.len()
    }

    // Encoded as body-less Viper method
    #[trusted]
    #[ensures(result.len() == length)]
    #[ensures(forall(|i: usize| (0 <= i && i < length) ==> result.lookup(i) == 0))]
    pub fn new(length: usize) -> Self {
        VecWrapperI32{ v: vec![0; length] }
    }

    // Encoded as body-less Viper function
    #[trusted]
    #[pure]
    #[requires(0 <= index && index < self.len())]
    pub fn lookup(&self, index: usize) -> i32 {
        self.v[index]
    }

    // Encoded as body-less Viper method
    #[trusted]
    #[requires(0 <= index && index < self.len())]
    #[ensures(self.lookup(old(index)) == old(value))]
    pub fn store(&mut self, index: usize, value: i32) {
        self.v[index] = value;
    }
}

struct List {
    head: Link,
}

enum Link {
    Empty,
    More,
}

#[trusted]
#[pure]
fn len(x: &Link) -> usize {
    unimplemented!()
}

#[trusted]
#[ensures(len(dest) > 123)]
fn replace(dest: &mut Link, src: ()) -> Link {
    unimplemented!()
}

fn pop(s: &mut Link) {
    replace(s, ());
}
