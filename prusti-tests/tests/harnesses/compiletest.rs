// © 2020, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use prusti_server::{
    save_verification_cache, spawn_server_thread, verification_cache_hits_and_misses,
};
use std::{
    env, fs,
    io::Write,
    num::NonZeroUsize,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};
use ui_test::{
    custom_flags::rustfix::RustfixMode,
    default_file_filter, default_per_file_config, run_tests_generic,
    spanned::Spanned,
    status_emitter::{Gha, StatusEmitter},
    AbortCheck, Args, Config,
};

/// The test groups, each with `fail`, `pass` and `ui` tests in `prusti-tests/tests/<group>/`, and
/// the environment that Prusti runs them in.
const GROUPS: [(&str, &[(&str, &str)]); 4] = [
    // Test the parsing of specifications. This doesn't run the verifier.
    ("parse", &[("PRUSTI_VERIFY_MODE", "compile-only")]),
    // Test the type-checking of specifications. This doesn't run the verifier.
    ("typecheck", &[("PRUSTI_VERIFY_MODE", "compile-only")]),
    // Test the verifier.
    ("verify", &[("PRUSTI_CHECK_OVERFLOWS", "false")]),
    // Test the verifier with overflow checks enabled.
    ("verify_overflow", &[]),
];

type TestsResult = ui_test::color_eyre::Result<()>;

/// The number of tests marked `//@ known-failure` that were run.
static KNOWN_FAILURES: AtomicUsize = AtomicUsize::new(0);

fn prusti_config(path: String, env: &[(&str, &str)], abort_check: &AbortCheck) -> Config {
    let mut config = Config::rustc(path);
    config.out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui");
    config.abort_check = abort_check.clone();
    config.program.program = super::tested_bin("prusti-rustc");
    config.program.args.push("-Awarnings".into());
    config
        .program
        .envs
        .push(("RUSTC_ICE".into(), Some("0".into()))); // suppress rustc-ice*.txt files
    config.program.envs.extend(
        [
            ("PRUSTI_FULL_COMPILATION", "true"),
            ("PRUSTI_QUIET", "true"),
        ]
        .iter()
        .chain(env)
        .map(|(k, v)| (k.into(), Some(v.into()))),
    );
    // `//@ known-failure` marks a test that should pass but currently fails
    // verification. Such a test succeeds as long as verification fails, and
    // fails once it verifies, so that the annotation gets removed. For
    // `fail` and `ui` tests, see `run`.
    config
        .custom_comments
        .insert("known-failure", |parser, _args, span| {
            KNOWN_FAILURES.fetch_add(1, Ordering::Relaxed);
            parser.exit_status = Spanned::new(1, span.clone()).into();
            parser.normalize_stderr.push((
                ui_test::Match::Regex(regex::bytes::Regex::new("(?s).*").unwrap()),
                b"".into(),
            ));
            parser.set_custom_once("no-rustfix", (), span);
        });
    config
}

fn per_file_config(config: &mut Config, file_contents: &Spanned<Vec<u8>>) {
    default_per_file_config(config, file_contents);
    // Expected outputs refer to the directory of the test file as `$DIR`.
    if let Some(dir) = file_contents.span.file.parent() {
        let dir = ui_test::Match::Exact(format!("{}/", dir.display()).into());
        let comments = config.comment_defaults.base();
        for normalize in [
            &mut comments.normalize_stderr,
            &mut comments.normalize_stdout,
        ] {
            normalize.push((dir.clone(), b"$DIR/".into()));
        }
    }
}

pub(crate) fn run() -> TestsResult {
    // Run from the repository root, so that reported test paths are relative to it.
    env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())?;

    // The server verifies in this process, which does not get the settings that `prusti-rustc`
    // passes to the driver. Without this one, `PRUSTI_USE_SMT_WRAPPER` makes it panic.
    env::set_var(
        "PRUSTI_SMT_SOLVER_WRAPPER_PATH",
        super::tested_bin("prusti-smt-solver"),
    );

    // Spawn server process as child (so it stays around until main function terminates)
    let server_address = spawn_server_thread();
    env::set_var("PRUSTI_SERVER_ADDRESS", server_address.to_string());

    let abort_check = AbortCheck::default();
    let ctrlc_abort_check = abort_check.clone();
    _ = ctrlc::try_set_handler(move || ctrlc_abort_check.abort());

    let args = Args::test()?;
    let mut configs = vec![];
    for (group_name, env) in GROUPS {
        let mut fail = prusti_config(
            format!("prusti-tests/tests/{group_name}/fail"),
            env,
            &abort_check,
        );
        fail.comment_defaults.base().normalize_stderr.push((
            ui_test::Match::Regex(regex::bytes::Regex::new(".*\n").unwrap()),
            b"".into(),
        ));
        let mut pass = prusti_config(
            format!("prusti-tests/tests/{group_name}/pass"),
            env,
            &abort_check,
        );
        pass.comment_defaults.base().exit_status = Spanned::dummy(0).into();
        pass.comment_defaults.base().require_annotations = Spanned::dummy(false).into();
        // A `ui` test checks Prusti's output against its `.stderr` and `.stdout` files, whether
        // or not it reports errors.
        let mut ui = prusti_config(
            format!("prusti-tests/tests/{group_name}/ui"),
            env,
            &abort_check,
        );
        ui.comment_defaults.base().exit_status = Default::default();
        ui.comment_defaults.base().require_annotations = Spanned::dummy(false).into();
        // The suggestions in a `ui` test's output are checked as part of its `.stderr` file, not
        // applied.
        ui.comment_defaults
            .base()
            .set_custom("rustfix", RustfixMode::Disabled);
        // Locations in the sources of the contracts, e.g. of the specification of a panicking
        // function, move whenever the contracts are edited, so the outputs show `LL:COL` in place
        // of their line and column.
        ui.filter(
            r"(prusti-contracts/src/\S+\.rs):[0-9]+:[0-9]+",
            "${1}:LL:COL",
        );
        // Unlike a `pass` test, a `ui` test marked `//@ known-failure` is still compared with its
        // `.stderr` and `.stdout` files, and is run like a known-failing `fail` test (see below).
        ui.custom_comments
            .insert("known-failure", |_parser, _args, _span| {
                KNOWN_FAILURES.fetch_add(1, Ordering::Relaxed);
            });
        configs.extend([fail, pass, ui]);
    }
    for config in &mut configs {
        config.with_args(&args);
        // Only `ui` tests have expected output files; `--bless` would write empty ones next to
        // the other tests.
        if !config.root_dir.ends_with("ui") {
            config.output_conflict_handling = ui_test::error_on_output_conflict;
        }
    }

    // A `fail` test marked `//@ known-failure` does not report the errors it is annotated with
    // yet, and a `ui` test marked so does not produce the output in its `.stderr` and `.stdout`
    // files yet. Such a test is run on its own afterwards and succeeds as long as it fails (for a
    // `ui` test: its output differs from these files), and fails once it passes, so that the
    // annotation gets removed.
    let is_known_failure = |path: &Path| {
        fs::read_to_string(path).is_ok_and(|contents| contents.contains("//@ known-failure"))
    };
    let known_failing_tests = Mutex::new(vec![]);
    let result = run_tests_generic(
        configs,
        |path, config| {
            let matched = default_file_filter(path, config)?;
            let rerun = config.root_dir.ends_with("fail") || config.root_dir.ends_with("ui");
            if matched && rerun && is_known_failure(path) {
                let mut config = config.clone();
                // `--bless` must not replace the expected output of a known failure with the
                // wrong output it produces.
                config.output_conflict_handling = ui_test::error_on_output_conflict;
                let test = (path.to_path_buf(), config);
                known_failing_tests.lock().unwrap().push(test);
                return Some(false);
            }
            Some(matched)
        },
        per_file_config,
        (
            Box::<dyn StatusEmitter>::from(args.format),
            Gha {
                name: "prusti-tests".to_string(),
                group: true,
            },
        ),
    );
    let mut fixed = vec![];
    for (path, config) in known_failing_tests.into_inner().unwrap() {
        let passed = run_tests_generic(
            vec![config],
            |candidate, _| Some(candidate == path),
            per_file_config,
            (),
        )
        .is_ok();
        println!(
            "{} ... {}",
            path.display(),
            if passed { "FIXED" } else { "known failure" }
        );
        if passed {
            // Reported like a failing `pass` test: annotated at the marker in the PR diff.
            let marker_line = fs::read_to_string(&path)?
                .lines()
                .position(|line| line.contains("//@ known-failure"))
                .and_then(|index| NonZeroUsize::new(index + 1));
            let error = ui_test::github_actions::error(
                path.display(),
                "test marked `//@ known-failure` now passes, remove the annotation",
            );
            drop(match marker_line {
                Some(line) => error.line(line),
                None => error,
            });
            fixed.push(path);
        }
    }

    let cache_usage = save_verification_cache().then(|| {
        let (hits, misses) = verification_cache_hits_and_misses();
        format!("Verification cache: {hits} hits, {misses} misses")
    });

    let known_failures = format!(
        "Test files still marked `//@ known-failure`: {}",
        KNOWN_FAILURES.load(Ordering::Relaxed)
    );
    println!("{known_failures}");
    if let Some(cache_usage) = &cache_usage {
        println!("{cache_usage}");
    }
    if let Some(mut summary) = ui_test::github_actions::summary() {
        writeln!(summary, "\n{known_failures}")?;
        if let Some(cache_usage) = &cache_usage {
            writeln!(summary, "\n{cache_usage}")?;
        }
        if !fixed.is_empty() {
            writeln!(summary, "### `//@ known-failure` tests that now pass")?;
            for path in &fixed {
                writeln!(summary, "* {}", path.display())?;
            }
        }
    }
    result?;
    if !fixed.is_empty() {
        ui_test::color_eyre::eyre::bail!(
            "tests marked `//@ known-failure` now pass, remove the annotation: {fixed:?}"
        );
    }
    Ok(())
}
