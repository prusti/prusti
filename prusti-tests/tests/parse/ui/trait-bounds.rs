//@ aux-build: trait_bounds_aux.rs
//@ compile-flags: -Pprint_desugared_specs=true -Pprint_typeckd_specs=true -Phide_uuids=true -Pverify_mode=compile-only
//@ normalize-stdout-test: "[a-z0-9]{32}" -> "$(NUM_UUID)"
//@ normalize-stdout-test: "[a-z0-9]{8}-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{12}" -> "$(UUID)"
//@ normalize-stdout-test: "\[[a-z0-9]{4}\]::" -> "[$(CRATE_ID)]::"
//@ normalize-stdout-test: "#\[prusti::specs_version = \x22.+\x22\]" -> "#[prusti::specs_version = $(SPECS_VERSION)]"

use trait_bounds_aux::Foo;
use prusti_contracts::*;

#[extern_spec]
impl<'a, T: PartialEq, const L: usize> Foo<'a, T, L> {
    #[trusted]
    #[pure]
    #[ensures(result == self.0)]
    pub fn bar(self) -> &'a [T; L];
}

fn main() {}
