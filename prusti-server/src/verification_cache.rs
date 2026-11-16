use prusti_utils::config;
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::Path,
};
use viper::PersistentCache;

/// Loads the verification cache stored at `cache_path`, if one is configured. Its results are
/// only reused by the same Prusti backend (`BACKEND_FINGERPRINT`, see `build.rs`) running the
/// same Z3 and Boogie.
pub(crate) fn load_verification_cache() -> PersistentCache {
    let cache_path = config::cache_path();
    // Without a path, the cache lives only as long as this process, whose tools do not change.
    let fingerprint = if cache_path.as_os_str().is_empty() {
        0
    } else {
        tools_fingerprint()
    };
    PersistentCache::load_cache(cache_path, fingerprint)
}

fn tools_fingerprint() -> u64 {
    let mut hasher = DefaultHasher::new();
    env!("BACKEND_FINGERPRINT").hash(&mut hasher);
    hash_file(&mut hasher, Path::new(&config::smt_solver_path()));
    if let Some(boogie_path) = config::boogie_path() {
        // Besides Boogie's own files, which are named after it, the directory of the Boogie
        // executable holds the .NET runtime that Boogie ships with.
        let boogie_dir = Path::new(&boogie_path).parent().unwrap();
        let mut boogie_files = fs::read_dir(boogie_dir)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", boogie_dir.display()))
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("Boogie"))
            })
            .collect::<Vec<_>>();
        boogie_files.sort();
        for file in boogie_files {
            file.file_name().hash(&mut hasher);
            hash_file(&mut hasher, &file);
        }
    }
    hasher.finish()
}

fn hash_file(hasher: &mut DefaultHasher, path: &Path) {
    fs::read(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        .hash(hasher);
}
