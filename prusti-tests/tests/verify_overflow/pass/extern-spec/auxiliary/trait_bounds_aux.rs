pub struct Foo<'a, T: PartialEq, const L: usize>(pub &'a [T; L]);

impl<'a, T: PartialEq, const L: usize> Foo<'a, T, L> {
    pub fn bar(self) -> &'a [T; L] { self.0 }
}
