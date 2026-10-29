use prusti_contracts::*;

pub fn test<T>(x: i8, y: T) -> T{ y }

fn instantiate_test(){
    let a = test(-127, 11);
}

struct Foo<T> {
    f: T,
}

fn foo<T>(a: Foo<T>, b: Foo<Foo<T>>) {}

fn nested_instances() {
    foo(
        Foo { f: 0 },
        Foo { f: Foo { f: 1 } },
    );
}

struct Container<T> {
    content: T,
}

#[requires(container.content == 123)]
#[ensures(result.content == 124)]
fn increment_container(container: Container<i32>) -> Container<i32> {
    Container { content: container.content + 1 }
}

#[ensures(result > 123)]
fn container_client() -> i32 {
    let num_container = Container { content: 123 };
    let incremented_container = increment_container(num_container);
    incremented_container.content
}

struct Pair<T, U> {
    first: T,
    second: U,
}

#[requires(pair.second == true)]
#[ensures(result.second == true)]
fn copy_pair<T>(pair: Pair<T, bool>) -> Pair<T, bool> {
    Pair {
        first: pair.first,
        second: pair.second
    }
}

fn fst<T, U>(pair: Pair<T, U>) -> T {
  let unused = pair.second;
  pair.first
}

#[ensures(result == true)]
fn pair_client() -> bool {
    let initial_pair = Pair {
        first: 42u32,
        second: true
    };
    let copied_pair = copy_pair(initial_pair);
    copied_pair.second
}

fn fst_client() {
    let pair = Pair { first: 1, second: 2 };
    fst(pair);
}

/*struct R(Option<Box<u32>>);
fn main() {
    match R(None).0 {
        Some(_) => (),
        _ => (),
    }
}*/
struct R(Option<u32>);
fn match_nested_option() {
    match R(None).0 {
        Some(_) => (),
        _ => (),
    }
}

fn boxes() {
    let x = Box::new(42);
    assert!(*x == 42);

    let y = Some(Box::new(72));
    match y {
        Some(n) => assert!(*n == 72),
        None => assert!(false),
    }
}
