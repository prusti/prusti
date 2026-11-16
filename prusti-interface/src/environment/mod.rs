// © 2019, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! This module defines the interface provided to a verifier.

use prusti_rustc_interface::middle::ty::TyCtxt;

pub mod body;
mod collect_closure_defs_visitor;
mod collect_prusti_spec_visitor;
mod diagnostic;
pub mod mir_storage;
mod name;
mod query;

pub use self::{
    body::EnvBody, collect_closure_defs_visitor::CollectClosureDefsVisitor,
    collect_prusti_spec_visitor::CollectPrustiSpecVisitor, diagnostic::EnvDiagnostic,
    name::EnvName, query::EnvQuery,
};
use crate::data::ProcedureDefId;

/// Facade to the Rust compiler.
pub struct Environment<'tcx> {
    prusti_version: &'static str,
    pub body: EnvBody<'tcx>,
    pub diagnostic: EnvDiagnostic<'tcx>,
    pub name: EnvName<'tcx>,
    pub query: EnvQuery<'tcx>,
}

impl<'tcx> Environment<'tcx> {
    /// Builds an environment given a compiler state.
    pub fn new(tcx: TyCtxt<'tcx>, prusti_version: &'static str) -> Self {
        Environment {
            prusti_version,
            body: EnvBody::new(tcx),
            diagnostic: EnvDiagnostic::new(tcx),
            name: EnvName::new(tcx),
            query: EnvQuery::new(tcx),
        }
    }

    /// Returns the typing context
    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.query.tcx()
    }

    /// Get ids of Rust procedures that are annotated with a Prusti specification
    #[tracing::instrument(level = "debug", skip(self))]
    pub fn get_annotated_procedures(&self) -> Vec<ProcedureDefId> {
        let mut visitor = CollectPrustiSpecVisitor::new(self);
        visitor.visit_all_item_likes();

        let mut cl_visitor = CollectClosureDefsVisitor::new(self);
        self.query
            .tcx()
            .hir_visit_all_item_likes_in_crate(&mut cl_visitor);

        let mut procedures = visitor.into_result();
        procedures.extend(cl_visitor.get_closure_defs());
        procedures
    }

    /// Compare the current version of the `prusti` crate to the given other version
    pub fn compare_prusti_version(&self, other: &str) -> std::cmp::Ordering {
        version_compare::compare(self.prusti_version, other)
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to compare Prusti version '{}' with other version '{other}'!",
                    self.prusti_version
                )
            })
            .ord()
            .unwrap()
    }

    /// Get the current version of the `prusti` crate
    pub fn get_prusti_version(&self) -> &'static str {
        self.prusti_version
    }

    /// Compare to the current version of the `prusti-specs` crate which we've been compiled with
    pub fn compare_to_curr_specs_version(other: &str) -> std::cmp::Ordering {
        let compiled_specs_version = prusti_specs::SPECS_VERSION;
        version_compare::compare(compiled_specs_version, other)
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to compare `prusti-specs` version '{}' with found version '{other}'!",
                    prusti_specs::SPECS_VERSION
                )
            })
            .ord()
            .unwrap()
    }

    /// Get the current version of the `prusti` crate
    pub fn get_specs_version() -> &'static str {
        prusti_specs::SPECS_VERSION
    }
}
