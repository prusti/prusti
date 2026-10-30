pub trait SomeTrait<T> {
    type AssocType;
    fn foo(&self, x: T) -> i32;
}
