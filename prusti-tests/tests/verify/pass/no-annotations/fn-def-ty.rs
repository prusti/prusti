use std::num::NonZero;

fn main() {
    let a = [1, 2, 3];
    let b = a.map(NonZero::new);
    let c = b.map(|x| x.map(NonZero::get));
}

fn higher_ranked_closure() {
    let _ = (0..1).filter(|_| true);
}
