use prusti_contracts::*;

trait SomeTrait {
    fn foo(x: i32) -> i32;
}
struct SomeStruct;
#[refine_trait_spec]
impl SomeTrait for SomeStruct {
    #[requires(false)] //~ ERROR: the implementation's precondition may be stronger than the trait method's
    fn foo(x: i32) -> i32 {
        panic!()
    }
}
