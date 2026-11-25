use prusti_contracts::*;

trait Tr<'a> {}

fn after_type_parameter<'a, 'b, T: Tr<'a>>(x: (T, &'b mut i32)) -> &'b mut i32 {
    x.1
}

fn main() {}
