use prusti_contracts::*;

#[pure]
fn id<T>(x: T) -> T {
    x
}

#[pure]
fn add_ids(){
    let x = id(1);
    let y = id(2);
    let z = x + y;
}

#[pure] fn test1() -> i32 { id(42) }
#[pure] fn test2() -> bool { id(true) }

#[ensures(test1() == 42)]
#[ensures(test2())]
fn instantiate_id() {}

struct Pair<T, U>(T, U);

#[pure]
fn pair<T, U>(x: T, y: U) -> Pair<T, U> {
    Pair(x, y)
}

#[ensures(pair(1, true).0 == 1)]
// #[ensures(pair(1, true).1 == true)]
fn pair_field() {}

#[pure]
#[trusted]
fn f<T, U>(t: T, u: U) -> T {
    t
}

#[pure]
fn g<U, V, W>(x: Pair<U, V>, w: W) -> Pair<U, V> {
   f(x, w)
}

fn call_mono(){
    f(Pair(1, 2), 3);
    g(Pair(4, 5), 6);
}

struct Point<T> {
    x: T,
    y: T,
}

impl<T: Copy> Point<T> {
    #[pure]
    fn new(x: T, y: T) -> Point<T> {
        Point { x, y }
    }

    #[pure]
    fn x(self) -> T {
        self.x
    }

    #[pure]
    fn y(self) -> T {
        self.y
    }
}

#[ensures(Point::new(1, 2).x() == 1)]
fn point_method() {}

struct Tuple2<T1, T2>(T1, T2);

struct Wrapper<T>(T);

#[pure]
fn insideWrapper<U>(u: Wrapper<U>) -> U { u.0 }

#[pure]
fn tupleInsideWrapper<U, V>(u: Wrapper<Tuple2<U, V>>) -> Tuple2<U, V> { insideWrapper(u) }


#[ensures(tupleInsideWrapper(Wrapper(Tuple2(7, 1))).0 == insideWrapper(Wrapper(7)))]
fn nested_wrapper_tuple() {

}

#[ensures(insideWrapper(Wrapper(Tuple2(0, 1))).1 == 1)]
fn wrapper_tuple_field() {

}

#[pure]
#[ensures(true)]
fn foo<T>(a: T) -> bool { true }

struct X <T, U> {a: T, b: U}

#[trusted]
#[pure]
fn get_i8() -> i8 {0}

#[trusted]
#[pure]
fn get_i16() -> i16 {1}

#[requires(foo(X {a: get_i8(), b: get_i16()}))]
#[requires(foo(X {a: get_i16(), b: get_i8()}))]
#[requires(foo(get_i8()))]
fn generic_struct_args() {
}
