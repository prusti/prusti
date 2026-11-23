# Binary stage

- The user invokes the binary `prusti-rustc <file.rs>`.
- The binary sets up important environment variables, then invokes `prusti_driver`.

This is a short stage responsible for setting up the correct environment variables for the Rust compiler and the Prusti-supporting libraries. To invoke `prusti-driver` correctly, the following arguments are set up:

 - `--sysroot`, pointing to the Rust sysroot where its libraries are stored.

When `prusti-rustc` is run directly, it also sets up the following arguments (under `cargo-prusti`, Cargo links the package's `prusti-contracts` dependency instead):

 - `-L`, adding the directories that contain the compiled `prusti-contracts` crate and its dependencies to the library search path.
 - `--extern prusti_contracts=...`, specifying the path of the compiled `prusti-contracts` library, which provides the procedural macros for Prusti specifications.
 - `--crate-type=lib`, unless a crate type is given, so that a file without a `main` function can be verified.

> - [`prusti-launch/src/bin/prusti-rustc.rs`](https://github.com/prusti/prusti/blob/main/prusti-launch/src/bin/prusti-rustc.rs)
