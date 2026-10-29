struct MyStruct {
    a: i32,
    b: (i32, i32)
}

fn foo<'a>(arg: MyStruct) -> (i32, i32) {
    let mut x = MyStruct { a: 111, b: (222, 333) };
    x.b.0 = 444;
    let y = x;
    let z = MyStruct { a: 555, b: (666, 777) };
    y.b
}

fn join<'a>(mut x: MyStruct, switch: bool) -> MyStruct {
    if switch {
        // Unfold MyStruct
        x.b = (444, 555);
    } else {
        // Nothing
    }
    // Join branches
    x
}

fn multiple_branches<'a>(switch: bool) -> MyStruct {
    let mut x = MyStruct { a: 000, b: (000, 000) };
    let y = MyStruct { a: 111, b: (222, 333) };
    if switch { return x }
    if switch { x.b.0 = 444; }
    if switch { x.b.1 = 555; }
    if switch { return x }
    if switch { x.a = 666; }
    if switch { x.b = (777, 888); }
    if switch { return x }
    if switch { x.a = y.a; }
    if switch { x.b = y.b; }
    if switch { x.b.0 = 999; }
    let z = x;
    z
}

#[derive(Clone,Copy)]
struct UnitBox {
    unit: ()
}

#[derive(Clone,Copy)]
struct MyBox {
    val: i32,
    ghost: UnitBox,
    dummy: UnitBox,
}

fn copy(x: MyBox) -> MyBox {
    let y = x;
    let z = x;
    y
}
