use std::{env, path::PathBuf};

pub(crate) mod cargotest;
pub(crate) mod compiletest;

/// The Prusti binary `name` that the tests run: the one in `TESTED_PRUSTI_BIN_DIR` if that is set
/// (e.g. to the release binaries that are published), otherwise the one that
/// `prusti-contracts-build` builds together with the contracts. The variable does not start with
/// `PRUSTI_` because Prusti rejects unknown `PRUSTI_*` variables, and the tests pass their
/// environment on to Prusti.
fn tested_bin(name: &str) -> PathBuf {
    let dir = env::var_os("TESTED_PRUSTI_BIN_DIR").map_or_else(
        || PathBuf::from(prusti_contracts_build::BIN_DIR),
        PathBuf::from,
    );
    dir.join(format!("{name}{}", env::consts::EXE_SUFFIX))
}
