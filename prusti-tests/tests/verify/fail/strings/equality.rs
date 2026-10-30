fn go(arg1: &str) -> &str {
    if arg1 == "f".to_string() {
       panic!();
       //~^ ERROR: might not hold
    } 
    arg1
}

fn main() {
 go("bar");
}
