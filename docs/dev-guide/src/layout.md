# Repository Layout

This section describes the crates located in the Prusti repository, their function, and key modules. Some important files are linked and described individually, although this list is far from complete.

## Binaries

These crates relate to the runnable executables produced during compilation.

 - [`prusti/`](https://github.com/prusti/prusti/tree/main/prusti) - `prusti-driver`, which invokes the Rust compiler with Prusti callbacks set up.
   - [`src/callbacks.rs`](https://github.com/prusti/prusti/blob/main/prusti/src/callbacks.rs) - the callbacks, which collect the specifications and start the verification.
 - [`prusti-launch/`](https://github.com/prusti/prusti/tree/main/prusti-launch) - the binaries that users run.
   - [`src/bin/prusti-rustc.rs`](https://github.com/prusti/prusti/blob/main/prusti-launch/src/bin/prusti-rustc.rs) - spawns `prusti-driver` with the correct environment.
   - [`src/bin/cargo-prusti.rs`](https://github.com/prusti/prusti/blob/main/prusti-launch/src/bin/cargo-prusti.rs) - runs Prusti on a Cargo package.
   - [`src/bin/prusti-server.rs`](https://github.com/prusti/prusti/blob/main/prusti-launch/src/bin/prusti-server.rs) - starts the [Prusti server](pipeline/viper.md#prusti-server).
 - [`prusti-server/`](https://github.com/prusti/prusti/tree/main/prusti-server) - [Prusti compilation server](pipeline/viper.md#prusti-server), and the code that verifies the encoded program, with or without the server.

## Specification parsing

These crates relate to parsing Prusti-specific specifications in Rust code (e.g. `requires`, `ensures`, etc). These specifications are defined using [procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html), which must be contained in their own crates, hence the additional `prusti-contracts/*` crates.

 - [`prusti-contracts/prusti-contracts/`](https://github.com/prusti/prusti/tree/main/prusti-contracts/prusti-contracts) - stubs for Rust pseudo-functions used in specifications; currently `old` and `before_expiry`. It also reexports the procedural macros `requires`, `ensures`, etc. Depending on the `prusti` feature, it exports either `impl` or `internal`.
 - [`prusti-contracts/prusti-contracts-proc-macros/`](https://github.com/prusti/prusti/tree/main/prusti-contracts/prusti-contracts-proc-macros) – a procedural macro crate that either forwards calls to `prusti-specs` (if the `prusti` feature is enabled) or tries to be invisible when compiling with the regular compiler (if the `prusti` feature is disabled). The `prusti` feature is set automatically when compiling with Prusti.
 - [`prusti-contracts/prusti-specs/`](https://github.com/prusti/prusti/tree/main/prusti-contracts/prusti-specs) - does the specification rewriting.
 - [`prusti-contracts/prusti-contracts-test`](https://github.com/prusti/prusti/tree/main/prusti-contracts/prusti-contracts-test) – a minimal test that checks that `prusti-contracts` can be used with a regular compiler.

## Common library code

These crates contain the majority of Prusti's code.

 - [`prusti-utils/`](https://github.com/prusti/prusti/tree/main/prusti-utils) - utilities used across Prusti (code independent from Rust internals), e.g. the [configuration flags](config/flags.md) and helpers for launching the binaries.
 - [`prusti-rustc-interface/`](https://github.com/prusti/prusti/tree/main/prusti-rustc-interface) - reexports of the Rust compiler crates used by Prusti.
 - [`prusti-interface/`](https://github.com/prusti/prusti/tree/main/prusti-interface) - wrapper around the Rust compiler internals that aims to provide a more stable interface (however, it fails to **completely** encapsulate the Rust compiler from its clients).
   - [`src/environment/`](https://github.com/prusti/prusti/tree/main/prusti-interface/src/environment) - most of the wrapper code lives here.
   - [`src/specs/`](https://github.com/prusti/prusti/tree/main/prusti-interface/src/specs) – collect type-checked specifications.
 - [`vir/`](https://github.com/prusti/prusti/tree/main/vir) - VIR, Prusti's intermediate representation of Viper programs.
 - [`task-encoder/`](https://github.com/prusti/prusti/tree/main/task-encoder) - the interface of task encoders, and the program they emit their outputs into.
 - [`prusti-encoder/`](https://github.com/prusti/prusti/tree/main/prusti-encoder) - MIR to VIR encoding.
   - [`src/encoders/`](https://github.com/prusti/prusti/tree/main/prusti-encoder/src/encoders) - the task encoders.
 - [`pcg/`](https://github.com/prusti/pcg) - the PCG (Place Capability Graph) analysis of MIR bodies, included as a Git submodule.
 - [`prusti-viper/`](https://github.com/prusti/prusti/tree/main/prusti-viper) - conversion of VIR to Viper AST.

## JVM bindings

- [`viper-sys`](https://github.com/prusti/prusti/tree/main/viper-sys) – low-level (unsafe) JVM bindings.
- [`viper`](https://github.com/prusti/prusti/tree/main/viper) – higher-level JVM bindings.

## Deprecated or removed

 - `prusti-common/` - common modules used across Prusti, including the VIR of the previous encoder.
 - `prusti-filter/` - walks through Rust crates to check which are supported by Prusti (used for evaluation in Prusti publication).
 - `prusti-macros/` - macros to simplify parsing code.
