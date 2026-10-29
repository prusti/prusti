fn foo(args: impl IntoIterator) { drop(args) }

fn foo_with_item(args: impl IntoIterator<Item = u32>) { drop(args) }
