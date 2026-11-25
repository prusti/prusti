//! This module allows storing mir bodies with borrowck facts in thread-local
//! storage.
//!
//! Unfortunately, thread-local storage requires all data stored in it to have
//! a `'static` lifetime. Therefore, we transmute the lifetime `'tcx` away when
//! storing the data. To ensure that nothing gets messed up, we require the
//! client to provide a witness: an instance of type `TyCtxt<'tcx>` that is
//! used to show that the lifetime that the client provided is indeed `'tcx`.

use crate::specs::is_spec_fn;
use prusti_rustc_interface::{
    borrowck::{consumers, consumers::BodyWithBorrowckFacts},
    data_structures::fx::FxHashMap,
    hir::def_id::LocalDefId,
    middle::{
        mir,
        queries::{mir_borrowck::ProvidedValue as MirBorrowck, Providers},
        ty::TyCtxt,
        util,
    },
};
use prusti_utils::config;
use std::{cell::RefCell, thread_local};

thread_local! {
    pub static SHARED_STATE_WITH_FACTS:
        RefCell<FxHashMap<LocalDefId, BodyWithBorrowckFacts<'static>>> =
        RefCell::new(FxHashMap::default());
}

/// # Safety
///
/// See the module level comment.
pub unsafe fn store_mir_body<'tcx>(
    _tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
    body_with_facts: BodyWithBorrowckFacts<'tcx>,
) {
    // SAFETY: See the module level comment.
    let body_with_facts: BodyWithBorrowckFacts<'static> =
        unsafe { std::mem::transmute(body_with_facts) };
    assert_eq!(
        body_with_facts.body.phase,
        mir::MirPhase::Analysis(mir::AnalysisPhase::Initial)
    );
    SHARED_STATE_WITH_FACTS.with(|state| {
        let mut map = state.borrow_mut();
        assert!(map.insert(def_id, body_with_facts).is_none());
    });
}

/// # Safety
///
/// See the module level comment.
#[allow(clippy::needless_lifetimes)] // We want to be very explicit about lifetimes here.
pub(crate) unsafe fn retrieve_mir_body<'tcx>(
    _tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
) -> BodyWithBorrowckFacts<'tcx> {
    let body_with_facts: BodyWithBorrowckFacts<'static> = SHARED_STATE_WITH_FACTS.with(|state| {
        let mut map = state.borrow_mut();
        map.remove(&def_id).unwrap_or_else(|| {
            panic!("No MIR body found for {def_id:?}");
        })
    });
    // SAFETY: See the module level comment.
    unsafe { std::mem::transmute(body_with_facts) }
}

/// Overrides the query that produces the MIR bodies so that it also stores
/// them here, where the encoder retrieves them.
pub fn override_queries(providers: &mut util::Providers) {
    providers.queries.mir_borrowck = mir_borrowck;
}

#[allow(clippy::needless_lifetimes)]
#[tracing::instrument(level = "debug", skip(tcx))]
fn mir_borrowck<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> MirBorrowck<'tcx> {
    // Like rustc's own `mir_borrowck`, skip trivial consts: they have no `mir_promoted`
    // body to borrow-check, and no closures whose bodies Prusti would need.
    if !tcx.is_trivial_const(def_id) {
        // Computing the Polonius input facts can be very slow, so they are computed only where
        // the PCG may need them: not for spec functions, and not in crates that are not verified.
        let consumer_opts =
            if is_spec_fn(tcx, def_id.to_def_id()) || config::verify_mode().is_compile_only() {
                consumers::ConsumerOptions::RegionInferenceContext
            } else {
                consumers::ConsumerOptions::PoloniusInputFacts
            };
        let body_with_facts = consumers::get_bodies_with_borrowck_facts(tcx, def_id, consumer_opts);
        for (def_id, body) in body_with_facts {
            // SAFETY: This is safe because we are feeding in the same `tcx` that is
            // going to be used as a witness when pulling out the data.
            unsafe {
                store_mir_body(tcx, def_id, body);
            }
        }
    }
    let mut providers = Providers::default();
    prusti_rustc_interface::borrowck::provide(&mut providers);
    let original_mir_borrowck = providers.mir_borrowck;
    original_mir_borrowck(tcx, def_id)
}
