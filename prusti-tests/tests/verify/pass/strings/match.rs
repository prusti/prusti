fn main(){
    match "bar" {
        "foo" => {},
        _ => {}
    }
}

fn compare() -> bool {
    if "foo" == "" {
        true
    } else {
        false
    }
}
