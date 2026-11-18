//! Builds `prusti-contracts` with the Prusti binaries of the current build; see `build.rs`.

/// The directory with the binaries that built `prusti-contracts`: `prusti-rustc`,
/// `cargo-prusti`, `prusti-driver` and `prusti-smt-solver`. They find the contracts, Viper and
/// Z3 relative to their own location, so they can run from there.
pub const BIN_DIR: &str = env!("PRUSTI_BIN_DIR");
