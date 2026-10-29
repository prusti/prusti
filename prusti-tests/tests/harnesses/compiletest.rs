// © 2020, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use prusti_server::spawn_server_thread;
use std::{
    env, fs,
    io::Write,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};
use ui_test::{
    default_file_filter, default_per_file_config, run_tests_generic,
    spanned::Spanned,
    status_emitter::{Gha, StatusEmitter},
    AbortCheck, Args, Config,
};

/// Silicon's time limit (in seconds) per test file, so that a slow test cannot hold up the suite.
const EXTRA_VERIFIER_ARGS: &str = "--timeout=90";

/// The test groups, each with `fail` and `pass` tests in `prusti-tests/tests/<group>/`, and the
/// environment that Prusti runs them in. The `ui` tests of each group are not run yet.
const GROUPS: [(&str, &[(&str, &str)]); 4] = [
    // Test the parsing of specifications. This doesn't run the verifier.
    ("parse", &[("PRUSTI_VERIFY_MODE", "compile-only")]),
    // Test the type-checking of specifications. This doesn't run the verifier.
    ("typecheck", &[("PRUSTI_VERIFY_MODE", "compile-only")]),
    // Test the verifier.
    (
        "verify",
        &[
            ("PRUSTI_CHECK_OVERFLOWS", "false"),
            ("PRUSTI_EXTRA_VERIFIER_ARGS", EXTRA_VERIFIER_ARGS),
        ],
    ),
    // Test the verifier with overflow checks enabled.
    (
        "verify_overflow",
        &[("PRUSTI_EXTRA_VERIFIER_ARGS", EXTRA_VERIFIER_ARGS)],
    ),
];

type TestsResult = ui_test::color_eyre::Result<()>;

/// The number of tests marked `//@ known-failure` that were run.
static KNOWN_FAILURES: AtomicUsize = AtomicUsize::new(0);

fn find_prusti_rustc_path() -> PathBuf {
    let target_directory = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let executable_name = if cfg!(windows) {
        "prusti-rustc.exe"
    } else {
        "prusti-rustc"
    };
    let local_prusti_rustc_path: PathBuf = ["target", target_directory, executable_name]
        .iter()
        .collect();
    if local_prusti_rustc_path.exists() {
        return local_prusti_rustc_path;
    }
    panic!(
        "Could not find the {target_directory:?} prusti-rustc binary to be used in tests. \
        It might be that Prusti has not been compiled correctly."
    );
}

fn prusti_config(path: String, env: &[(&str, &str)], abort_check: &AbortCheck) -> Config {
    let mut config = Config::rustc(path);
    config.out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui");
    config.abort_check = abort_check.clone();
    config.program.program = find_prusti_rustc_path();
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
    // `fail` tests, see `run`.
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

pub(crate) fn run() -> TestsResult {
    // Run from the repository root, so that reported test paths are relative to it.
    env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())?;

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
        configs.extend([fail, pass]);
    }
    for config in &mut configs {
        config.with_args(&args);
    }

    // A `fail` test marked `//@ known-failure` does not report the errors it is annotated with
    // yet. Such a test is run on its own afterwards and succeeds as long as it fails, and fails
    // once it passes, so that the annotation gets removed.
    let is_known_failure = |path: &Path| {
        fs::read_to_string(path).is_ok_and(|contents| contents.contains("//@ known-failure"))
    };
    let known_failing_fail_tests = Mutex::new(vec![]);
    let result = run_tests_generic(
        configs,
        |path, config| {
            let matched = default_file_filter(path, config)?;
            if matched && config.root_dir.ends_with("fail") && is_known_failure(path) {
                let test = (path.to_path_buf(), config.clone());
                known_failing_fail_tests.lock().unwrap().push(test);
                return Some(false);
            }
            Some(matched)
        },
        default_per_file_config,
        (
            Box::<dyn StatusEmitter>::from(args.format),
            Gha {
                name: "prusti-tests".to_string(),
                group: true,
            },
        ),
    );
    let mut fixed = vec![];
    for (path, config) in known_failing_fail_tests.into_inner().unwrap() {
        let passed = run_tests_generic(
            vec![config],
            |candidate, _| Some(candidate == path),
            default_per_file_config,
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

    let known_failures = format!(
        "Test files still marked `//@ known-failure`: {}",
        KNOWN_FAILURES.load(Ordering::Relaxed)
    );
    println!("{known_failures}");
    if let Some(mut summary) = ui_test::github_actions::summary() {
        writeln!(summary, "\n{known_failures}")?;
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
