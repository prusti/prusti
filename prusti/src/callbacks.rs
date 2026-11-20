use crate::verifier::verify;
use ::log::{debug, info};
use ide::{fake_error, IdeInfo};
use prusti_interface::{
    data::VerificationTask,
    environment::{mir_storage, Environment},
    specs::{self, cross_crate::CrossCrateSpecs},
    PrustiError,
};
use prusti_rustc_interface::{
    ast::{AttrVec, Crate, Item, ItemKind, Visibility, VisibilityKind, DUMMY_NODE_ID},
    driver::Compilation,
    interface::{interface::Compiler, Config},
    middle::ty::TyCtxt,
    session::Session,
    span::{Ident, Symbol, DUMMY_SP},
};
use prusti_utils::config;

#[derive(Default)]
pub struct PrustiCompilerCalls;

struct NoAnn;

impl prusti_rustc_interface::ast_pretty::pprust::PpAnn for NoAnn {}

impl prusti_rustc_interface::driver::Callbacks for PrustiCompilerCalls {
    fn config(&mut self, config: &mut Config) {
        // With incremental compilation, rustc does not run the `mir_borrowck` provider below for
        // a body that has not changed since the last session, so it would not store its MIR.
        config.opts.incremental = None;
        assert!(config.override_queries.is_none());
        config.override_queries = Some(|_session: &Session, providers| {
            mir_storage::override_queries(providers);
        });
    }

    fn after_crate_root_parsing(&mut self, compiler: &Compiler, krate: &mut Crate) -> Compilation {
        // `prusti-rustc` will pass a `--extern` option indicating where to find
        // the `prusti-contracts` crate. This crate must be loaded in order to
        // include the default extern specs (for e.g build-in `assert!`
        // statements). However, if the user does not explicitly import the
        // crate in the code or use an `extern crate` statement, then the
        // compiler will not load the crate.
        //
        // To work around this, we add a dummy `extern crate` statement to the
        // crate if one does not already exist.
        //
        // Ideally we could load the prusti-contracts crate directly in the
        // TyCtxt rather than modifying the crate AST.  However, it does not
        // seem possible to load the crate directly in the TyCtxt in the
        // `after_expansion` or `after_analysis` callback as the `cstore` object
        // appears to be frozen at that point.

        if compiler.sess.opts.externs.get("prusti_contracts").is_some() {
            let prusti_contracts_symbol = Symbol::intern("prusti_contracts");

            if !krate.items.iter().any(|item| match item.kind {
                ItemKind::ExternCrate(Some(original_ident), _) => {
                    original_ident == prusti_contracts_symbol
                }
                ItemKind::ExternCrate(None, ident) => ident.name == prusti_contracts_symbol,
                _ => false,
            }) {
                krate.items.push(Box::new(Item {
                    attrs: AttrVec::new(),
                    id: DUMMY_NODE_ID,
                    span: DUMMY_SP,
                    vis: Visibility {
                        kind: VisibilityKind::Public,
                        span: DUMMY_SP,
                    },
                    kind: ItemKind::ExternCrate(
                        None,
                        Ident::new(prusti_contracts_symbol, DUMMY_SP),
                    ),
                    tokens: None,
                }));
            }
        }
        Compilation::Continue
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn after_expansion<'tcx>(&mut self, compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if compiler.sess.is_rust_2015() {
            compiler
                .sess
                .dcx()
                .struct_warn(
                    "Prusti specifications are supported only from 2018 edition. Please \
                    specify the edition with adding a command line argument `--edition=2018` or \
                    `--edition=2021`.",
                )
                .emit();
        }
        compiler.sess.dcx().abort_if_errors();
        if config::print_desugared_specs() {
            // based on the implementation of rustc_driver::pretty::print_after_parsing
            let sess = &compiler.sess;
            let krate = &tcx.resolver_for_lowering().1.borrow();
            let src_name = sess.io.input.file_name(sess);
            let src = sess
                .source_map()
                .get_source_file(&src_name)
                .expect("get_source_file")
                .src
                .as_ref()
                .expect("src")
                .to_string();
            print!(
                "{}",
                prusti_rustc_interface::ast_pretty::pprust::print_crate(
                    sess.source_map(),
                    krate,
                    src_name,
                    src,
                    &NoAnn,
                    false,
                    sess.edition(),
                    &sess.psess.attr_id_generator,
                )
            );
        }
        Compilation::Continue
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn after_analysis<'tcx>(&mut self, compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        compiler.sess.dcx().abort_if_errors();
        let mut env = Environment::new(tcx, env!("CARGO_PKG_VERSION"));
        let spec_checker = specs::checker::SpecChecker::new();
        spec_checker.check(&env);
        compiler.sess.dcx().abort_if_errors();

        let mut spec_collector = specs::SpecCollector::new(&mut env);
        spec_collector.collect_specs();

        let mut def_spec = spec_collector.build_def_specs();
        // Do print_typeckd_specs prior to importing cross crate
        if config::print_typeckd_specs() {
            for value in def_spec.all_values_debug(config::hide_uuids()) {
                println!("{value}");
            }
        }
        CrossCrateSpecs::import_export_cross_crate(&mut env, &mut def_spec);

        // TODO: can we replace `get_annotated_procedures` with information
        // that is already in `def_spec`?
        let annotated_procedures = env.get_annotated_procedures();

        if config::show_ide_info() && !config::verify_mode().is_compile_only() {
            let compiler_info = IdeInfo::collect(&env, &annotated_procedures, &def_spec);
            let out = serde_json::to_string(&compiler_info).unwrap();
            PrustiError::message(format!("compilerInfo{out}"), DUMMY_SP.into())
                .emit(&env.diagnostic);
        }
        // as long as we have to throw a fake error we need to check this
        // note that is_single_file will still be false if this is run through x.py.
        // it will find a manifest in prusti-launch but CARGO_PRIMARY_PACKAGE will still
        // not be ok. meaning that selective verification can't currently be tested through
        // x.py if the target is a single-file program.
        let is_single_file = std::env::var("CARGO_MANIFEST_DIR").is_err();
        let is_primary_package = is_single_file || std::env::var("CARGO_PRIMARY_PACKAGE").is_ok();

        // collect and output Information used by IDE:
        if !config::verify_mode().is_compile_only() && !config::skip_verification() {
            let target_def_paths = config::verify_only_defpaths();
            debug!(
                "Received def paths: {target_def_paths:?}. Package is primary: {is_primary_package}, Package is single-file: {is_single_file}"
            );
            if !target_def_paths.is_empty() {
                // if we do selective verification, then definitely only
                // for methods of the primary package. Check needed because
                // of the fake_error, otherwise verification stops early
                // with local dependencies
                if is_primary_package {
                    let procedures = annotated_procedures
                        .into_iter()
                        .filter(|x| target_def_paths.contains(&env.name.get_unique_item_name(*x)))
                        .collect::<Vec<_>>();
                    if !procedures.is_empty() {
                        let selective_task = VerificationTask {
                            procedures,
                            selective: true,
                        };
                        verify(env, def_spec, selective_task);
                    } else {
                        info!("Passed selective defpaths were empty or had no matching procedures - skipping verification.");
                    }
                    // fake_error(&env_diagnostic);
                }
            } else {
                let verification_task = VerificationTask {
                    procedures: annotated_procedures,
                    selective: false,
                };
                verify(env, def_spec, verification_task);
            }
        } else if config::skip_verification()
            && !config::verify_mode().is_compile_only()
            && is_primary_package
        {
            // add a fake error, reason explained in issue #1261
            fake_error(&env.diagnostic);
        }

        compiler.sess.dcx().abort_if_errors();
        if config::full_compilation() {
            Compilation::Continue
        } else {
            Compilation::Stop
        }
    }
}
