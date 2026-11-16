//! Computes `BACKEND_FINGERPRINT`, which identifies everything compiled into Prusti that
//! determines the result of verifying a VIR program: the Viper jars, and the source of the
//! code that lowers VIR to Viper, runs the verifier and reads its results back. Results in a
//! persistent verification cache are only reused by a build with the same fingerprint.

use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::Path,
};

/// Paths relative to this crate that hold code or dependency versions affecting verification
/// results. A directory stands for every file in it.
const BACKEND_SOURCES: &[&str] = &[
    "src",
    "../vir/src",
    "../vir-proc-macro/src",
    "../prusti-viper/src",
    "../viper/src",
    "../viper-sys/src",
    "../viper-sys/build.rs",
    "../jni-gen/src",
    "../prusti-smt-solver/src",
    "../smt-log-analyzer/src",
    "../Cargo.lock",
    "../rust-toolchain",
];

fn main() {
    let mut hasher = DefaultHasher::new();
    // `viper-sys` generates its bindings from the jars in `VIPER_HOME`, and Prusti runs Viper
    // from the same jars.
    println!("cargo:rerun-if-env-changed=VIPER_HOME");
    let viper_home = env::var("VIPER_HOME").expect("failed to get VIPER_HOME");
    println!("cargo:rerun-if-changed={viper_home}");
    hash_path(&mut hasher, Path::new(&viper_home), Path::new(&viper_home));
    for source in BACKEND_SOURCES {
        println!("cargo:rerun-if-changed={source}");
        hash_path(&mut hasher, Path::new(source), Path::new(source));
    }
    println!(
        "cargo:rustc-env=BACKEND_FINGERPRINT={:016x}",
        hasher.finish()
    );
}

/// Hashes the contents of the file or directory `path` together with the paths of its files
/// relative to `root`, so that moving a file changes the hash but moving the checkout does not.
fn hash_path(hasher: &mut DefaultHasher, root: &Path, path: &Path) {
    if path.is_dir() {
        let mut entries = fs::read_dir(path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            hash_path(hasher, root, &entry);
        }
    } else {
        path.strip_prefix(root).unwrap().hash(hasher);
        fs::read(path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
            .hash(hasher);
    }
}
