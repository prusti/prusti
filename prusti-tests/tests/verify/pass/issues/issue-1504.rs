// Regression test for prusti/prusti#1504: string constants passed to println.
use prusti_contracts::*;

const MAX_FIBONACCI: u32 = 2_971_215_073;

pub fn play_game(n: u32) {
    println!("{}", fizz_buzz_fibonacci(n));
}

#[trusted]
fn is_fibonacci_number(n: u32) -> bool {
    n <= MAX_FIBONACCI
}

pub fn fizz_buzz_fibonacci(n: u32) -> String {
    if is_fibonacci_number(n) {
        "Fibonacci".to_string()
    } else {
        match (n % 3, n % 5) {
            (0, 0) => "FizzBuzz".to_string(),
            (0, _) => "Fizz".to_string(),
            (_, 0) => "Buzz".to_string(),
            (_, _) => n.to_string(),
        }
    }
}

#[ensures(result == n)]
pub fn print_and_return(n: u32) -> u32 {
    println!("value: {}", n);
    n
}
