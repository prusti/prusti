use prusti_contracts::*;

trait TraitWithAssocType {
    type Item;
    fn foo(&self) -> Option<Self::Item>;
}


#[extern_spec]
trait TraitWithAssocType {
    type Item = i32; //~ ERROR: Associated types in external trait specs should not be declared

    #[ensures(result == 42)]
    fn foo(&self) -> Option<Self::Item>;
}

trait TraitWithDefaultMethod {
    fn foo(&self) -> i32 {
        42
    }
}


#[extern_spec]
trait TraitWithDefaultMethod {
    #[ensures(result == 42)]
    fn foo(&self) -> i32 { //~ ERROR: Unexpected method body. (Extern specs only define specifications.)
        43
    }
}

/// External traits
trait GenericTrait<T> {
    fn get_value(&self) -> T;
}
/// External traits

#[extern_spec]
trait GenericTrait<#[concrete] i32> {
    //~^ ERROR: The `#[concrete]` and `#[generic]` attributes are deprecated. To refine specs for specific concrete types, use type-conditional spec refinements instead.
    #[ensures(result == 42)]
    fn get_value(&self) -> i32;
}

#[extern_spec]
trait GenericTrait<#[generic] T> {
    //~^ ERROR: The `#[concrete]` and `#[generic]` attributes are deprecated. To refine specs for specific concrete types, use type-conditional spec refinements instead.
    #[ensures(true)]
    fn get_value(&self) -> T;
}
