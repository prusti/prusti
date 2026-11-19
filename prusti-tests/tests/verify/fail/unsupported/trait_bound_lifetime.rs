use prusti_contracts::*;

trait Tr<'a> {}

fn after_type_parameter<'a, 'b, T: Tr<'a>>(x: (T, &'b mut i32)) -> &'b mut i32 { //~ ERROR: lifetimes in the trait bounds of a type parameter are not supported where other lifetimes follow the type parameter
    x.1
}

fn main() {}
