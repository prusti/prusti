# Prusti processing stage

- The MIR of functions that should be checked is obtained.
- MIR is encoded into VIR - Prusti's intermediate representation.
- `fold`/`unfold` statements and other ghost code are derived from the PCG.
- VIR is simplified.

> - [`prusti/src/verifier.rs` - `verify`](https://github.com/prusti/prusti/blob/main/prusti/src/verifier.rs) - function combining the steps in this stage.

## MIR and VIR

Prusti starts with Rust's [MIR (mid-level intermediate representation)](https://rustc-dev-guide.rust-lang.org/mir/index.html). The MIR is a [CFG](https://en.wikipedia.org/wiki/Control-flow_graph)-based representation that encodes Rust in a highly simplified (and therefore easier to programmatically process) manner. Prusti overrides the compiler's `mir_borrowck` query to store the MIR bodies together with their borrow-checker facts, which the encoder reads back.

The output of this stage is VIR (Viper intermediate representation). The VIR mirrors the Viper AST closely, except that method bodies are CFGs of basic blocks, which become Viper labels and `goto`s. The conversion of VIR to Viper AST is straight-forward by design.

> - [`prusti/src/callbacks.rs` - `mir_borrowck`](https://github.com/prusti/prusti/blob/main/prusti/src/callbacks.rs) - stores the MIR bodies.
> - [`prusti-interface/src/environment/body.rs` - `EnvBody`](https://github.com/prusti/prusti/blob/main/prusti-interface/src/environment/body.rs) - access to the stored MIR bodies.
> - [`vir/src/gendata.rs`](https://github.com/prusti/prusti/blob/main/vir/src/gendata.rs) - definitions of most VIR data structures, e.g. expressions.

## Finding functions to check

In this step, functions to be checked are found by visiting the HIR with a visitor that looks for Prusti-defined attributes.

> - [`prusti-interface/src/environment/mod.rs` - `Environment::get_annotated_procedures`](https://github.com/prusti/prusti/blob/main/prusti-interface/src/environment/mod.rs)

## Encoding MIR to VIR

In this step, Rust MIR is encoded to VIR. The encoding is split into task encoders, each of which encodes one kind of item (e.g. a method, a pure function or a type) and requests the encodings of the items it depends on from other encoders. Pure functions are treated separately, since they may be used in specifications. The details of this encoding are complex, but a high-level overview can be found in the [Prusti publication, Appendix A](http://pm.inf.ethz.ch/publications/getpdf.php?bibname=Own&id=AstrauskasMuellerPoliSummers19.pdf#appendix.A).

> - [`task-encoder/src/lib.rs` - `TaskEncoder`](https://github.com/prusti/prusti/blob/main/task-encoder/src/lib.rs) - the interface implemented by all task encoders.
> - [`prusti-encoder/src/lib.rs` - `test_entrypoint`](https://github.com/prusti/prusti/blob/main/prusti-encoder/src/lib.rs) - encodes all functions of the crate and collects the outputs of all encoders into one program.
> - [`prusti-encoder/src/encoders/mir_fn/method.rs` - `MethodEnc`](https://github.com/prusti/prusti/blob/main/prusti-encoder/src/encoders/mir_fn/method.rs) - encoder for impure functions.
> - [`prusti-encoder/src/encoders/mir_impure.rs`](https://github.com/prusti/prusti/blob/main/prusti-encoder/src/encoders/mir_impure.rs) - encoding of the bodies of impure functions.

## Ghost code and simplification

The `fold`/`unfold` statements, magic wands and other ghost code of impure functions are derived from the results of the PCG (Place Capability Graph) analysis of their MIR, while their bodies are encoded. Once all items are encoded, the program is simplified, unless the [`simplify_encoding`](../config/flags.md#simplify_encoding) flag is turned off.

> - [`pcg/`](https://github.com/prusti/pcg) - the PCG analysis, included as a Git submodule.
> - [`vir/src/simplify/mod.rs`](https://github.com/prusti/prusti/blob/main/vir/src/simplify/mod.rs) - simplification of the encoded program.
