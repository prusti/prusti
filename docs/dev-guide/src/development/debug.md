# Debugging

The `x.py` script can be used to run Prusti on an individual file:

```bash
$ ./x.py run --bin prusti-rustc path/to/the/file.rs
```

See the list of [flags](../config/flags.md) for way to configure Prusti. Of particular use when debugging are:

 - [`PRINT_DESUGARED_SPECS`](../config/flags.md#print_desugared_specs)
 - [`PRUSTI_DUMP_VIPER_PROGRAM`](../config/flags.md#dump_viper_program)
 - [`PRUSTI_LOG`](../config/flags.md#log)

## Debugging tests

A proposed way of fixing Prusti limitations is to add a regression test by creating a new file in `prusti-tests/tests` and then use the following command to run it and collect Prusti's debugging output:

```bash
RUST_BACKTRACE=1 PRUSTI_DUMP_VIPER_PROGRAM=true PRUSTI_DUMP_DEBUG_INFO=true \
./x.py ++verbose verify-test <path-to-the-test-file>
```

This command does the following:

1. Runs the specified test through the test harness, like `./x.py test --test main` does. So Prusti is built and run as for the test suite: with the compiler options and the environment of the test's group and the flags specified in the test file. The variables that the harness sets in `prusti-tests/tests/harnesses/compiletest.rs` (for all tests, for the test's group in `GROUPS`, and for its own verification server) replace those of the same name in your environment. CI additionally sets the variables in the `env` of `.github/workflows/test.yml` and Silicon's time limits, which `.github/actions/time-limits` sets in `PRUSTI_ASSERT_TIMEOUT` and `PRUSTI_EXTRA_VERIFIER_ARGS`. The harness prints Prusti's output only if the test fails, so never for a test marked `//@ known-failure` that still fails; to see its output, remove the marker while debugging.
2. Dumps the information that is most commonly needed for debugging Prusti into the `log` directory.

Tip: to see the expanded Prusti specification macros, add the following comment at the beginning of the test file:

```rust
//@ compile-flags: -Zunpretty=expanded
```

### Running Prusti in a debugger

The `verify-test` command also generates a VS Code launch config, which allows you to run Prusti in a debugger. It runs `prusti-driver` with the arguments and the variables that `prusti-rustc` passed to it, but not with the environment the driver inherited, such as the variables of the test's group.

- In the `Cargo.toml`, section `[profile.dev]`, set `debug = 2`. This enables debug symbols.
- Install the CodeLLDB VS Code extension.

Now you can set breakpoints and launch the debugger.

### Debugging performance problems

In case you are debugging a performance problem and suspect that it is caused by quantifiers, append `--analyze-quantifiers` to the command after the `<path-to-the-test-file>`. This will run Z3 in tracing mode and produce CSV files with various statistics in `log/smt` directory. In case you want to see all terms that were used to trigger a specific quantifier, you can produce the list of them with the following command:

```bash
PRUSTI_SMT_TRACE_QUANTIFIER_TRIGGERS=<quantifier-id> \
./x.py ++verbose run --release --bin smt-log-analyzer log/smt/<function>/trace1.log
```

You can find the list of quantifier ids and names in `log/smt/<function>/trace1.log.unique-triggers.csv`. Running the `smt-log-analyzer` will generate `log/smt/<function>/trace1.log.quantifier-<quantifier-id>-triggers.csv` file containing all triggers used to instantiate the quantifier.
