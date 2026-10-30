fn foo(p: *const i32) {
    let _ = p.is_null();
}

fn main() {
    let _: *const i32 = std::ptr::null_mut();
}
