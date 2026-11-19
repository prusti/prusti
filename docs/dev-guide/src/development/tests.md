# Tests

Tests for the Prusti project can be executed using the Python wrapper script:

```bash
./x.py test
```

The script sets up environment variables just like in the [build](build.md) phase, then runs `cargo test`. Prusti's test files (in `prusti-tests/tests`) are run with the Prusti binaries that `prusti-contracts-build` builds together with the contracts, so `./x.py test` always tests the current code. The tests of `prusti-launch` run the binaries in `target/debug` instead, and `./x.py test` does not update `target/debug/prusti-driver`; build Prusti with `./x.py build` before running them.

Any additional arguments are added to the `cargo test` command line. For example:

```bash
$ ./x.py test --all --verbose
```

## Filtering tests

The [options available](https://doc.rust-lang.org/book/ch11-02-running-tests.html) when running `cargo test` still apply. For example, to only run tests which contain `mod` in their name:

```bash
$ ./x.py test mod
```

To run only Prusti's test files, select the `main` test target of `prusti-tests` and pass the paths:

```bash
$ ./x.py test --test main -- tests/verify/pass/arrays/
```

`./x.py build` and `./x.py test` share one build, so neither compiles again what the other already compiled.

## UI tests

The tests in the `ui` directory of each test group check Prusti's output: the harness compares what a test prints with the `.stderr` and `.stdout` files next to it, in which the test's directory is written as `$DIR` and locations in the sources of `prusti-contracts` have `LL:COL` in place of their line and column, so that editing the contracts does not change these files. To write the current output to these files, pass `--bless`:

```bash
$ ./x.py test --test main -- --bless tests/verify/ui/warnings.rs
```

The harness compiles every test with `-Awarnings`, so a test that checks a warning, also one of Prusti's, enables them again with `//@ compile-flags: -Wwarnings`.

A test marked `//@ known-failure` shows a bug or a missing feature: it succeeds as long as it fails and fails once it passes, so that the marker gets removed. For a `ui` test, failing means that its output differs from its files, which `--bless` then leaves unchanged. Removing the marker line shifts the line numbers in the output, so bless the test again afterwards. Some known failures are tests of features that do not exist yet; their files keep the output of an older Prusti, which the test can never match, so once the feature works, remove the marker and bless the test.

## Additional flags

Some tests require setting additional flags, for example, to ensure that the number of quantifier instantiations is below a certain bound. This can be achieved by adding at the top of the test file `//@ compile-flags:` followed by the [list of arguments](../config/flags.md) to Prusti. For example, the following comment would bound the number of unique triggers used to instantiate a quantifier to 20:

```rust
//@ compile-flags: -Psmt_unique_triggers_bound=20
```

## Debugging tests

See [the debugging section](../development/debug.md).

