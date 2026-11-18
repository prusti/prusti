# Tests

Tests for the Prusti project can be executed using the Python wrapper script:

```bash
./x.py test
```

The script sets up environment variables just like in the [build](build.md) phase, then runs `cargo test`. Build Prusti with `./x.py build` first: Prusti's test files are run with the binaries in `target/debug`, which `./x.py test` does not update.

Any additional arguments are added to the `cargo test` command line. For example:

```bash
$ ./x.py test --all --verbose
```

## Filtering tests

The [options available](https://doc.rust-lang.org/book/ch11-02-running-tests.html) when running `cargo test` still apply. For example, to only run tests which contain `mod` in their name:

```bash
$ ./x.py test mod
```

To run only Prusti's test files (in `prusti-tests/tests`), build Prusti, then run `main`, the test target of `prusti-tests` that runs these files, with the paths to run:

```bash
$ ./x.py build
$ ./x.py test --test main -- tests/verify/pass/arrays/
```

`./x.py build` and `./x.py test` share one build, so the second command compiles only what the tests add, such as the test harness.

## Additional flags

Some tests require setting additional flags, for example, to ensure that the number of quantifier instantiations is below a certain bound. This can be achieved by adding at the top of the test file `// compile-flags:` followed by the [list of arguments](../config/flags.md) to Prusti. For example, the following comment would bound the number of unique triggers used to instantiate a quantifier to 20:

```rust
// compile-flags: -Psmt_unique_triggers_bound=20
```

## Debugging tests

See [the debugging section](../development/debug.md).

