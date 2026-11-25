//! Helpers for the unit tests of the encoders.

use prusti_interface::{
    environment::{EnvBody, mir_storage},
    specs::typed::DefSpecificationMap,
};
use prusti_rustc_interface::{
    driver::{self, Compilation},
    hir::def::DefKind,
    interface::{Config, interface::Compiler},
    middle::ty::TyCtxt,
    session::config::Input,
    span::FileName,
};
use std::{path::Path, sync::OnceLock};

use vir::{Foldable, simplify::SimplifyCtx};

use crate::encoders::ty::RustTyDecomposition;

/// Compiles `src` as a library and calls `f` after the analysis, with the
/// encoder's context set up. As in Prusti, the MIR bodies are kept for the
/// encoder.
pub(crate) fn with_crate<R: Send>(
    src: &str,
    f: impl for<'tcx> FnOnce(TyCtxt<'tcx>) -> R + Send,
) -> R {
    struct Callbacks<F, R> {
        src: String,
        f: Option<F>,
        result: Option<R>,
    }

    impl<F, R> driver::Callbacks for Callbacks<F, R>
    where
        F: for<'tcx> FnOnce(TyCtxt<'tcx>) -> R + Send,
        R: Send,
    {
        fn config(&mut self, config: &mut Config) {
            config.input = Input::Str {
                name: FileName::Custom("test.rs".to_string()),
                input: self.src.clone(),
            };
            config.override_queries = Some(|_, providers| {
                mir_storage::override_queries(providers);
            });
        }

        fn after_analysis(&mut self, _: &Compiler, tcx: TyCtxt<'_>) -> Compilation {
            vir::init_vcx(vir::VirCtxt::new(
                tcx,
                EnvBody::new(tcx),
                DefSpecificationMap::new(),
                vir::IdentStyle::ItemName,
            ));
            let f = self.f.take().unwrap();
            self.result = Some(f(tcx));
            Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
        src: src.to_string(),
        f: Some(f),
        result: None,
    };
    let args = [
        "rustc",
        "test.rs",
        "--crate-type=lib",
        "--edition=2021",
        "--cap-lints=allow",
    ];
    driver::run_compiler(&args.map(String::from), &mut callbacks);
    callbacks.result.unwrap()
}

/// Compiles `src`, which defines the type alias `X`, and calls `f` with the
/// aliased type, decomposed in the generic context of `X`.
pub(super) fn with_type_alias<R: Send>(
    src: &str,
    f: impl for<'tcx> FnOnce(RustTyDecomposition<'tcx>) -> R + Send,
) -> R {
    with_crate(src, |tcx| {
        let alias = tcx
            .hir_crate_items(())
            .definitions()
            .find(|&def_id| {
                tcx.def_kind(def_id) == DefKind::TyAlias && tcx.item_name(def_id).as_str() == "X"
            })
            .expect("the input defines no type alias `X`")
            .to_def_id();
        let ty = tcx
            .type_of(alias)
            .instantiate_identity()
            .skip_normalization();
        f(RustTyDecomposition::from_ty(ty, alias))
    })
}

/// The expression `e` as it appears in the final program, which is simplified.
pub(crate) fn simplified<'vir, E: Foldable<'vir, (), !> + Copy>(e: E) -> E {
    vir::with_vcx(|vcx| {
        let ctx = SimplifyCtx::new(&[], &[], &[], &[], &Default::default());
        e.fold_with(&mut vir::simplify::Simplify::new(vcx, &ctx))
            .unwrap_or(e)
    })
}

/// `s` without its first and last line and without the indentation that its
/// lines have in common, so that expected output can be written as an
/// indented raw string.
pub(crate) fn dedent(s: &str) -> String {
    let lines = s.lines().collect::<Vec<_>>();
    let lines = &lines[1..lines.len() - 1];
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|line| line.get(indent..).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Compiles `src`, which defines the type alias `X`, calls `encode` with the
/// aliased type, and verifies the program of everything encoded, followed by
/// the Viper code `viper`, with Silicon. Returns Silicon's error messages if
/// the verification fails.
///
/// All tests share one JVM. The program is kept in `target/viper-tests`,
/// named after the test. Silicon gets it as text, while Prusti passes an
/// AST, so a verdict could in principle differ from Prusti's.
pub(super) fn verify(
    src: &str,
    encode: impl for<'tcx> FnOnce(RustTyDecomposition<'tcx>) + Send,
    viper: &str,
) -> Result<(), String> {
    static VIPER: OnceLock<viper::Viper> = OnceLock::new();
    thread_local! {
        static COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    let program = with_type_alias(src, |ty| {
        encode(ty);
        let mut program = crate::emit_program();
        program.simplify();
        program.code()
    });
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/viper-tests");
    std::fs::create_dir_all(&dir).unwrap();
    let test = std::thread::current()
        .name()
        .unwrap_or("test")
        .replace("::", "-");
    let count = COUNT.with(|count| count.replace(count.get() + 1));
    let path = dir.join(format!("{test}-{count}.vpr"));
    std::fs::write(&path, format!("{program}\n{viper}")).unwrap();
    let z3 = std::env::var("Z3_EXE").expect("Z3_EXE is not set, run the tests with ./x.py");
    // The arguments that Prusti passes to Silicon, see
    // `prusti-server/src/verification_request.rs`.
    let args = [
        "--z3Exe",
        &z3,
        "--disableTerminationPlugin",
        "--proverConfigArgs",
        "smt.qi.eager_threshold=1000 model.partial=false smt.arith.solver=6",
        "--logLevel",
        "ERROR",
        path.to_str().unwrap(),
    ]
    .map(String::from);
    VIPER
        .get_or_init(viper::Viper::new_for_tests)
        .attach_current_thread()
        .verify_file_with_silicon(&args)
        .map_err(|errors| format!("{}\n(program: {})", errors.join("\n"), path.display()))
}
