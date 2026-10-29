# Working on Prusti

Prusti encodes Rust MIR into Viper and verifies the result with Silicon and Z3. The
developer guide (`docs/dev-guide`) covers setup, the architecture and the configuration
flags; this file collects working norms and pitfalls.

## Building and running

- Run every cargo command through `./x.py` (`./x.py build`, `./x.py test`, ...) from the
  repository root. It sets up the Java, Viper and Z3 environment that plain `cargo` lacks.
- Rebuild with `./x.py build` before running Prusti or the tests after a change.
  `./x.py run --bin prusti-rustc` and `./x.py test` do not update
  `target/debug/prusti-driver`, the binary that `prusti-rustc` runs and that contains the
  encoder, so they silently use the old encoder.
- Do not rebuild while tests or verification runs are in progress: processes started while
  the binaries are being replaced can fail in confusing ways.
- If results change inexplicably after switching branches, delete `target/verify`, which
  holds the `prusti-contracts` build and its `.specs` files.
- CI checks formatting with `./x.py fmt-check-all` and lints with
  `./x.py clippy -- --no-deps -D warnings`; format with `./x.py fmt-all`.
- Prusti panics on an unknown, non-empty `PRUSTI_*` environment variable, so give ad-hoc
  debugging variables a different prefix.
- `prusti-rustc` runs `prusti-driver` as a child process. Ctrl-C stops both, but a
  `timeout` or `kill` aimed at `prusti-rustc` leaves the driver and its Z3 processes
  running.

## Tests

- To reproduce a test outside the harness, use the environment and flags that
  `prusti-tests/tests/harnesses/compiletest.rs` sets, plus the file's `//@ compile-flags`
  (and, to match CI, the variables set in `.github/workflows/test.yml`). Otherwise the
  results differ, e.g. overflow errors appear for checks the harness turns off.
- Before writing a test, look at the existing tests of the feature. If one already covers
  the behaviour, don't add another. Otherwise add the case to an existing file where it
  fits, and create a new file only when none does (e.g. nothing tests that feature yet,
  or the case needs different flags).
- The full suite is slow. While iterating, run the affected files individually and leave
  the full suite to CI; compare against `main`'s CI results instead of producing a local
  baseline.
- To find which tests an encoding change affects, dump the Viper programs of the tests
  with `PRUSTI_VERIFY_MODE=encode-only PRUSTI_DUMP_VIPER_PROGRAM=true` on both builds and
  diff them. Every dump is written to the same file name, so give each test its own
  `PRUSTI_LOG_DIR`, and normalise non-deterministic parts such as spec-item UUIDs.
- Hard proofs are marginal: they can flip after unrelated changes to the same program.
  Don't judge a change by one such test.

## Soundness

- A passing test may be vacuous. When a change makes something verify that did not
  before, check it: put `prusti_assert!(false)` where the property is proved (at the end
  of the function, not at its start), or better assert the negation of the property.
  Prusti reports at most one error per method, so put each probe in its own function.
  A contradiction in quantified axioms only shows up where their triggers fire, so probe
  in code that uses them; an empty `#[ensures(false)]` function catches little.
- Treat an insufficient-permission error on safe Rust as an encoding bug.
- Report every encoding failure as an error at its span. Whatever stands in for the
  unsupported part must not assume anything: a placeholder with an `ensures false`
  contract makes everything after it verify, and a silently missing axiom resurfaces as
  an obscure verification error elsewhere.
- An axiom must hold wherever it can be instantiated: guard it by the conditions it
  depends on (e.g. an impl's where-clauses).

## Debugging verification

- Dump the program with `PRUSTI_DUMP_VIPER_PROGRAM=true` and run Silicon on it directly
  (`viper.silicon.SiliconRunner` in `viper_tools/backends/viperserver.jar`) with the
  arguments Prusti passes (`prusti-server/src/verification_request.rs`); with other
  arguments the verdict can differ. Keep `--logLevel ERROR` among them: Silicon's default
  level logs debug output, which on large programs grows to gigabytes.
  `PRUSTI_SHORT_VIPER_NAMES=true` makes dumps readable
  but can cause name collisions, so don't trust verdicts obtained with it.
- Bisect by editing the dump: an `assert false` that verifies marks an inconsistent state,
  replacing an axiom or contract clause with `true` removes it, and `--includeMethods`
  restricts which methods are verified (functions are still checked). Prusti hands
  Silicon an AST, not the dumped text, so confirm the conclusion through Prusti itself.
- For slow or `unknown` results, have Silicon log its prover interaction
  (`--proverLogFile <path>` in `PRUSTI_EXTRA_VERIFIER_ARGS`; it writes one file per
  prover instance, so add `--numberOfParallelVerifiers 1`) and replay the log with
  `z3 smt.qi.profile=true`. If the JVM is busy while Z3 idles, the time is spent in
  Silicon, not in the solver.

## Viper semantics that shape the encoding

- `old(e)` only changes the heap reads written inside `e`. A variable, including a
  `let`-bound one, keeps its current value under `old`, so the heap read itself must sit
  inside the `old`.
- A wand can only be applied if a wand of the same shape is held; differing
  subexpressions must be provably equal.
- Viper ADTs have no acyclicity axiom, so a constraint like `t == C(t)` is not refuted
  automatically.

## Encoder conventions

- In `TaskEncoder` impls, `emit_outputs` comes after `do_encode_full` (a few older
  encoders differ; don't copy them).
- An encoder's `emit_outputs` must be reachable from the `emit_outputs` calls in
  `prusti-encoder/src/lib.rs`; otherwise its output silently never reaches the program.
- Only the encoder that emits a Viper function or domain constructs identifiers referring
  to it; other encoders obtain them from its output, which ties using an identifier to
  encoding the item.
- Two different task keys must not emit the same Viper identifier: whatever distinguishes
  the keys has to appear in the name, or the keys have to be canonical. For the same
  reason a type's interning identity must not contain facts that depend on its
  instantiation.
- `AlreadyEncoded` means a re-entrant run already finished the current task; propagate
  it instead of unwrapping.
- An error handler must be registered on the span of the expression that can fail or on
  an enclosing one; backtranslation only follows span ancestors.
- If the PCG's analysis result is wrong (e.g. a missing or misordered action), fix it in
  the PCG instead of compensating in the encoder.

## Design

- Fix the root cause with a general mechanism. If a fix needs a condition per case, or
  review keeps finding new cases, the model is wrong.
- Before changing the encoding to make a failure go away, reduce it to a minimal Viper
  example and explain it; then propose options. For larger designs, write down the
  requirements first and check every part of the proposal against them.
- The standard library gets its meaning from extern specs in `prusti-contracts`, the same
  mechanism users have for specifying other libraries. When a std item's behaviour cannot
  be specified yet, extend the specification language rather than the encoder. An
  extension should be a concept that stands on its own: its meaning can be explained
  without the item that motivated it, user code with the same kind of behaviour would be
  specified with it too, and it composes with the existing features. An annotation that
  just names the peculiarity of one item is a special case in disguise. Likewise, when
  std items get poor treatment (e.g. unhelpful errors inside macro expansions), improve
  the general mechanism rather than matching on the item.
- Prefer the simpler change: no speculative generality, and no new code for a review
  finding that does not show a real failure.
- Before implementing something rustc may already implement, check the rustc source at
  the pinned toolchain and use what it provides, the way its API intends it to be used.

## Code and comments

- Avoid churn: change only what the change needs. No renamed locals, inlined bindings,
  reflowed lines or reshuffled code that the change does not require; cleanups belong in
  their own PR. Moved code stays textually identical. A shorter diff must not cost idiom.
- Simplifying the code you are working on is part of the work. If code elsewhere could
  become substantially simpler (logic implemented twice, a special case a more general
  mechanism could absorb, dead code), mention it in a line or two when you report back,
  without changing it; cosmetic nits are not worth raising.
- Comments are documentation of the code as it is, for whoever reads it next. They say
  what the code cannot: what an item is for, which invariant it keeps, why a non-obvious
  choice is correct. Each comment should make sense on its own, in complete sentences
  that name the actual things, and read as if written for the current code; don't
  restate the code. Match the comment density of the surrounding code, and use ASCII
  only.

## Pull requests

- Give branches descriptive names; no tool prefixes such as `claude/`.
- A PR that changes the generated Viper shows a minimal Rust example with the relevant
  part of its Viper encoding before and after, trimmed to what the change affects.
  Details that don't matter for the point can be simplified (elide unrelated statements,
  shorten incidental names), but keep the names of what the PR is about exactly as
  emitted, so readers can search for them in the code and in their own dumps.
- Review comments on someone else's PR may be about anything, however small, but each one
  must be right: a mistaken comment sends the author after a problem that does not exist
  and undermines trust in every other comment.
  Verify each claim before posting it: check API and behaviour claims against the source
  at the toolchain's commit, and check who wrote the code you criticise. Leave out what
  you cannot verify, or ask it as a question.
- Unless the change is trivial, have it reviewed right before opening the PR. Get the
  whole diff reviewed by a fresh, independent reviewer (e.g. a subagent using a code
  review skill), one round at a time. Check every finding against the code yourself
  before acting on it, fix the real ones, and rerun the affected tests. Give each next
  reviewer the findings already fixed or rejected, so they are not raised again. Stop
  after the first round that brings up nothing real; if a few rounds in they still find
  new problems, stop and ask the user. Finish with one read of the diff and the
  description as a human reviewer would see them.
