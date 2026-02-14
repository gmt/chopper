use crate::arg_validation::{self, ArgViolation};
use crate::env_util;
use crate::env_validation::{self, EnvKeyViolation, EnvValueViolation};
use crate::journal_validation::{self, JournalIdentifierViolation, JournalNamespaceViolation};
use crate::manifest::Manifest;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const CACHE_ENTRY_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceFingerprint {
    pub source_path: PathBuf,
    pub source_len: u64,
    pub source_modified_ns: u128,
    pub source_changed_ns: u128,
    pub source_device: u64,
    pub source_inode: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    version: u32,
    fingerprint: SourceFingerprint,
    manifest: Manifest,
}

pub fn source_fingerprint(path: &Path) -> Result<SourceFingerprint> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to stat alias config {}", path.display()))?;
    let modified = metadata
        .modified()
        .and_then(|time| {
            time.duration_since(UNIX_EPOCH)
                .map_err(std::io::Error::other)
        })
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let (source_changed_ns, source_device, source_inode) = unix_file_signature(&metadata);

    Ok(SourceFingerprint {
        source_path: path.to_path_buf(),
        source_len: metadata.len(),
        source_modified_ns: modified,
        source_changed_ns,
        source_device,
        source_inode,
    })
}

pub fn load(alias: &str, fingerprint: &SourceFingerprint) -> Option<Manifest> {
    let primary_path = cache_path(alias);
    if let Some(manifest) = load_from_path(&primary_path, fingerprint) {
        return Some(manifest);
    }

    let legacy_path = legacy_cache_path(alias);
    if legacy_path != primary_path {
        if let Some(manifest) = load_from_path(&legacy_path, fingerprint) {
            if store(alias, fingerprint, &manifest).is_ok() {
                delete_cache_file_best_effort(&legacy_path);
            }
            return Some(manifest);
        }
    }

    None
}

fn load_from_path(path: &Path, fingerprint: &SourceFingerprint) -> Option<Manifest> {
    let bytes = fs::read(path).ok()?;
    let entry: CacheEntry = match bincode::deserialize(&bytes) {
        Ok(entry) => entry,
        Err(_) => {
            delete_cache_file_best_effort(path);
            return None;
        }
    };
    if entry.version != CACHE_ENTRY_VERSION {
        delete_cache_file_best_effort(path);
        return None;
    }
    if entry.fingerprint == *fingerprint {
        if validate_cached_manifest(&entry.manifest).is_ok() {
            Some(entry.manifest)
        } else {
            delete_cache_file_best_effort(path);
            None
        }
    } else {
        delete_cache_file_best_effort(path);
        None
    }
}

fn validate_cached_manifest(manifest: &Manifest) -> Result<()> {
    if path_contains_nul(&manifest.exec) {
        return Err(anyhow!(
            "cached manifest exec path cannot contain NUL bytes"
        ));
    }

    for arg in &manifest.args {
        if matches!(
            arg_validation::validate_arg_value(arg),
            Err(ArgViolation::ContainsNul)
        ) {
            return Err(anyhow!("cached manifest args cannot contain NUL bytes"));
        }
    }

    for (key, value) in &manifest.env {
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            return Err(anyhow!(
                "cached manifest env keys cannot be empty or whitespace-only"
            ));
        }
        if normalized_key != key {
            return Err(anyhow!(
                "cached manifest env keys cannot include surrounding whitespace"
            ));
        }
        match env_validation::validate_env_key(normalized_key) {
            Ok(()) => {}
            Err(EnvKeyViolation::ContainsEquals) => {
                return Err(anyhow!("cached manifest env keys cannot contain `=`"));
            }
            Err(EnvKeyViolation::ContainsNul) => {
                return Err(anyhow!("cached manifest env keys cannot contain NUL bytes"));
            }
        }
        if matches!(
            env_validation::validate_env_value(value),
            Err(EnvValueViolation::ContainsNul)
        ) {
            return Err(anyhow!(
                "cached manifest env values cannot contain NUL bytes"
            ));
        }
    }

    for key in &manifest.env_remove {
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            return Err(anyhow!(
                "cached manifest env_remove keys cannot be empty or whitespace-only"
            ));
        }
        if normalized_key != key {
            return Err(anyhow!(
                "cached manifest env_remove keys cannot include surrounding whitespace"
            ));
        }
        match env_validation::validate_env_key(normalized_key) {
            Ok(()) => {}
            Err(EnvKeyViolation::ContainsEquals) => {
                return Err(anyhow!(
                    "cached manifest env_remove keys cannot contain `=`"
                ));
            }
            Err(EnvKeyViolation::ContainsNul) => {
                return Err(anyhow!(
                    "cached manifest env_remove keys cannot contain NUL bytes"
                ));
            }
        }
    }

    if let Some(journal) = &manifest.journal {
        match journal_validation::normalize_namespace(&journal.namespace) {
            Ok(normalized) => {
                if normalized != journal.namespace {
                    return Err(anyhow!(
                        "cached manifest journal namespace cannot include surrounding whitespace"
                    ));
                }
            }
            Err(JournalNamespaceViolation::Empty) => {
                return Err(anyhow!("cached manifest journal namespace cannot be empty"));
            }
            Err(JournalNamespaceViolation::ContainsNul) => {
                return Err(anyhow!(
                    "cached manifest journal namespace cannot contain NUL bytes"
                ));
            }
        }
        match journal_validation::normalize_optional_identifier_for_invocation(
            journal.identifier.as_deref(),
        ) {
            Ok(normalized_identifier) => {
                if let (Some(original), Some(normalized)) =
                    (journal.identifier.as_ref(), normalized_identifier)
                {
                    if original != &normalized {
                        return Err(anyhow!(
                            "cached manifest journal identifier cannot include surrounding whitespace"
                        ));
                    }
                }
            }
            Err(JournalIdentifierViolation::Blank) => {
                return Err(anyhow!(
                    "cached manifest journal identifier cannot be blank when provided"
                ));
            }
            Err(JournalIdentifierViolation::ContainsNul) => {
                return Err(anyhow!(
                    "cached manifest journal identifier cannot contain NUL bytes"
                ));
            }
        }
    }

    if let Some(reconcile) = &manifest.reconcile {
        if path_contains_nul(&reconcile.script) {
            return Err(anyhow!(
                "cached manifest reconcile script cannot contain NUL bytes"
            ));
        }

        let function = reconcile.function.trim();
        if function.is_empty() {
            return Err(anyhow!(
                "cached manifest reconcile function cannot be empty or whitespace-only"
            ));
        }
        if function != reconcile.function {
            return Err(anyhow!(
                "cached manifest reconcile function cannot include surrounding whitespace"
            ));
        }
        if function.contains('\0') {
            return Err(anyhow!(
                "cached manifest reconcile function cannot contain NUL bytes"
            ));
        }
    }

    Ok(())
}

#[cfg(unix)]
fn path_contains_nul(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().contains(&0)
}

#[cfg(not(unix))]
fn path_contains_nul(path: &Path) -> bool {
    path.to_string_lossy().contains('\0')
}

pub fn store(alias: &str, fingerprint: &SourceFingerprint, manifest: &Manifest) -> Result<()> {
    let path = cache_path(alias);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create cache directory {}", parent.display()))?;
    }

    let entry = CacheEntry {
        version: CACHE_ENTRY_VERSION,
        fingerprint: fingerprint.clone(),
        manifest: manifest.clone(),
    };
    let bytes = bincode::serialize(&entry).context("failed to serialize alias cache entry")?;
    write_atomically(&path, &bytes)
}

fn cache_path(alias: &str) -> PathBuf {
    let cache_dir = cache_dir();
    let (safe_alias, needs_hash_suffix) = sanitized_alias(alias);
    let filename = if !needs_hash_suffix {
        format!("{safe_alias}.bin")
    } else {
        format!("{safe_alias}-{:016x}.bin", alias_cache_hash(alias))
    };

    cache_dir.join("manifests").join(filename)
}

fn legacy_cache_path(alias: &str) -> PathBuf {
    let cache_dir = cache_dir();
    let safe_alias = sanitize_alias_for_cache(alias);
    cache_dir
        .join("manifests")
        .join(format!("{safe_alias}.bin"))
}

fn sanitized_alias(alias: &str) -> (String, bool) {
    let safe_alias = sanitize_alias_for_cache(alias);
    let needs_hash_suffix = safe_alias != alias;
    (safe_alias, needs_hash_suffix)
}

fn sanitize_alias_for_cache(alias: &str) -> String {
    alias
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn alias_cache_hash(alias: &str) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in alias.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

pub fn cache_dir() -> PathBuf {
    if let Some(override_path) = env_util::env_path_override("CHOPPER_CACHE_DIR") {
        return override_path;
    }

    directories::ProjectDirs::from("", "", "chopper")
        .map(|d| d.cache_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".chopper-cache"))
}

fn delete_cache_file_best_effort(path: &Path) {
    let _ = fs::remove_file(path);
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    const MAX_TMP_NAME_ATTEMPTS: usize = 32;

    for attempt in 0..MAX_TMP_NAME_ATTEMPTS {
        let tmp_path = cache_temp_path(path, attempt);
        let mut tmp_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to create temporary alias cache file {}",
                        tmp_path.display()
                    )
                });
            }
        };

        if let Err(err) = tmp_file.write_all(bytes) {
            delete_cache_file_best_effort(&tmp_path);
            return Err(err).with_context(|| {
                format!(
                    "failed to write temporary alias cache file {}",
                    tmp_path.display()
                )
            });
        }

        if let Err(err) = fs::rename(&tmp_path, path) {
            delete_cache_file_best_effort(&tmp_path);
            return Err(err).with_context(|| {
                format!("failed to finalize alias cache file {}", path.display())
            });
        }

        return Ok(());
    }

    Err(anyhow!(
        "failed to create temporary alias cache file for {}; too many filename collisions",
        path.display()
    ))
}

fn cache_temp_path(path: &Path, attempt: usize) -> PathBuf {
    path.with_extension(format!("tmp-{}-{attempt}", std::process::id()))
}

#[cfg(unix)]
fn unix_file_signature(metadata: &fs::Metadata) -> (u128, u64, u64) {
    use std::os::unix::fs::MetadataExt;

    let changed_ns = (metadata.ctime() as i128)
        .saturating_mul(1_000_000_000)
        .saturating_add(metadata.ctime_nsec() as i128)
        .max(0) as u128;
    (changed_ns, metadata.dev(), metadata.ino())
}

#[cfg(not(unix))]
fn unix_file_signature(_metadata: &fs::Metadata) -> (u128, u64, u64) {
    (0, 0, 0)
}

#[cfg(test)]
mod tests {
    use super::{
        cache_path, cache_temp_path, legacy_cache_path, load, sanitize_alias_for_cache,
        source_fingerprint, store, CacheEntry, CACHE_ENTRY_VERSION,
    };
    use crate::manifest::{JournalConfig, Manifest, ReconcileConfig};
    use crate::test_support::ENV_LOCK;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn cache_round_trip_and_invalidation() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");

        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");
        let manifest = Manifest::simple(PathBuf::from("echo"));
        store("demo", &fingerprint, &manifest).expect("store cache");

        let cached = load("demo", &fingerprint).expect("cache hit");
        assert_eq!(cached.exec, PathBuf::from("echo"));

        fs::write(&source_file, "exec = \"printf\"\n").expect("rewrite source");
        let new_fingerprint = source_fingerprint(&source_file).expect("new fingerprint");
        assert!(load("demo", &new_fingerprint).is_none());
        let cache_file = cache_path("demo");
        assert!(
            !cache_file.exists(),
            "fingerprint-mismatched cache file should be pruned"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn corrupted_cache_entry_is_ignored() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("fingerprint");

        let cache_file = home.path().join("chopper/manifests/broken.bin");
        fs::create_dir_all(
            cache_file
                .parent()
                .expect("cache file should have a parent directory"),
        )
        .expect("create cache dir");
        fs::write(&cache_file, [0, 159, 146, 150]).expect("write invalid cache bytes");

        assert!(load("broken", &fingerprint).is_none());
        assert!(
            !cache_file.exists(),
            "corrupted cache file should be pruned after read failure"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cache_path_honors_chopper_cache_override() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("CHOPPER_CACHE_DIR", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");
        let manifest = Manifest::simple(PathBuf::from("echo"));
        store("demo", &fingerprint, &manifest).expect("store cache");

        let expected = home.path().join("manifests").join("demo.bin");
        assert!(
            expected.exists(),
            "expected override cache file at {:?}",
            expected
        );
        env::remove_var("CHOPPER_CACHE_DIR");
    }

    #[test]
    fn blank_cache_override_falls_back_to_xdg_cache_root() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());
        env::set_var("CHOPPER_CACHE_DIR", "   ");

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");
        let manifest = Manifest::simple(PathBuf::from("echo"));
        store("demo-blank", &fingerprint, &manifest).expect("store cache");

        let expected = home.path().join("chopper/manifests/demo-blank.bin");
        assert!(
            expected.exists(),
            "blank override should fall back to XDG cache root: {:?}",
            expected
        );
        env::remove_var("CHOPPER_CACHE_DIR");
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn source_fingerprint_captures_unix_signature() {
        let source_home = TempDir::new().expect("create tempdir");
        let source_file = source_home.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");

        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");
        assert!(fingerprint.source_len > 0);
        #[cfg(unix)]
        {
            assert!(fingerprint.source_changed_ns > 0);
            assert!(fingerprint.source_device > 0);
            assert!(fingerprint.source_inode > 0);
        }
    }

    #[test]
    fn cache_entry_version_mismatch_invalidates_entry() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");
        let manifest = Manifest::simple(PathBuf::from("echo"));
        store("demo", &fingerprint, &manifest).expect("store cache");

        let path = cache_path("demo");
        let bytes = fs::read(&path).expect("read stored cache");
        let mut entry: CacheEntry = bincode::deserialize(&bytes).expect("deserialize entry");
        entry.version = CACHE_ENTRY_VERSION + 1;
        fs::write(
            &path,
            bincode::serialize(&entry).expect("re-serialize entry"),
        )
        .expect("rewrite cache entry");

        assert!(load("demo", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "version-mismatched cache file should be pruned"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn store_survives_preexisting_temporary_filename_collision() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");
        let manifest = Manifest::simple(PathBuf::from("echo"));

        let path = cache_path("demo");
        let parent = path.parent().expect("cache path parent");
        fs::create_dir_all(parent).expect("create cache parent");
        let colliding_tmp = cache_temp_path(&path, 0);
        fs::write(&colliding_tmp, b"stale").expect("write colliding temp file");

        store("demo", &fingerprint, &manifest).expect("store cache despite collision");
        assert!(path.exists(), "final cache file should exist");
        let loaded = load("demo", &fingerprint).expect("load stored cache");
        assert_eq!(loaded.exec, PathBuf::from("echo"));
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cache_alias_sanitization_replaces_unsafe_characters() {
        let safe = sanitize_alias_for_cache("alpha/beta\\gamma:delta space\tnewline\nemoji🚀");
        assert_eq!(safe, "alpha_beta_gamma_delta_space_newline_emoji_");
    }

    #[test]
    fn cache_path_disambiguates_aliases_that_sanitize_to_same_name() {
        let alias_a = "demo/prod";
        let alias_b = "demo:prod";
        let path_a = cache_path(alias_a);
        let path_b = cache_path(alias_b);

        assert_ne!(
            path_a, path_b,
            "sanitized collisions should be hash-disambiguated"
        );
        let file_a = path_a
            .file_name()
            .and_then(|name| name.to_str())
            .expect("cache filename should be utf-8");
        let file_b = path_b
            .file_name()
            .and_then(|name| name.to_str())
            .expect("cache filename should be utf-8");
        assert!(file_a.starts_with("demo_prod-"), "{file_a}");
        assert!(file_b.starts_with("demo_prod-"), "{file_b}");
        assert!(file_a.ends_with(".bin"), "{file_a}");
        assert!(file_b.ends_with(".bin"), "{file_b}");
    }

    #[test]
    fn safe_alias_cache_path_remains_unhashed() {
        let path = cache_path("demo-prod");
        let file = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("cache filename should be utf-8");
        assert_eq!(file, "demo-prod.bin");
    }

    #[test]
    fn load_migrates_legacy_cache_path_for_unsafe_aliases() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");
        let manifest = Manifest::simple(PathBuf::from("echo"));
        let alias = "alpha:beta";

        let legacy_path = legacy_cache_path(alias);
        let hashed_path = cache_path(alias);
        assert_ne!(
            legacy_path, hashed_path,
            "legacy and hashed paths should differ"
        );
        fs::create_dir_all(legacy_path.parent().expect("legacy parent"))
            .expect("create cache parent");
        let entry = CacheEntry {
            version: CACHE_ENTRY_VERSION,
            fingerprint: fingerprint.clone(),
            manifest: manifest.clone(),
        };
        fs::write(
            &legacy_path,
            bincode::serialize(&entry).expect("serialize legacy cache entry"),
        )
        .expect("write legacy cache file");

        let loaded = load(alias, &fingerprint).expect("load from legacy cache");
        assert_eq!(loaded.exec, manifest.exec);
        assert!(hashed_path.exists(), "expected migrated hashed cache file");
        assert!(
            !legacy_path.exists(),
            "expected legacy cache file to be pruned after successful migration"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_invalid_runtime_strings_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.args = vec!["ok".to_string(), "bad\0arg".to_string()];

        let path = cache_path("unsafe");
        fs::create_dir_all(path.parent().expect("cache path parent")).expect("create cache dir");
        let entry = CacheEntry {
            version: CACHE_ENTRY_VERSION,
            fingerprint: fingerprint.clone(),
            manifest,
        };
        fs::write(
            &path,
            bincode::serialize(&entry).expect("serialize cache entry"),
        )
        .expect("write cache file");

        assert!(load("unsafe", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached manifest should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_blank_journal_identifier_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.journal = Some(JournalConfig {
            namespace: "ops".to_string(),
            stderr: true,
            identifier: Some("   ".to_string()),
        });

        let path = cache_path("unsafe-journal");
        fs::create_dir_all(path.parent().expect("cache path parent")).expect("create cache dir");
        let entry = CacheEntry {
            version: CACHE_ENTRY_VERSION,
            fingerprint: fingerprint.clone(),
            manifest,
        };
        fs::write(
            &path,
            bincode::serialize(&entry).expect("serialize cache entry"),
        )
        .expect("write cache file");

        assert!(load("unsafe-journal", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached journal config should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_whitespace_journal_namespace_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.journal = Some(JournalConfig {
            namespace: " ops ".to_string(),
            stderr: true,
            identifier: None,
        });

        let path = cache_path("unsafe-journal-namespace");
        fs::create_dir_all(path.parent().expect("cache path parent")).expect("create cache dir");
        let entry = CacheEntry {
            version: CACHE_ENTRY_VERSION,
            fingerprint: fingerprint.clone(),
            manifest,
        };
        fs::write(
            &path,
            bincode::serialize(&entry).expect("serialize cache entry"),
        )
        .expect("write cache file");

        assert!(load("unsafe-journal-namespace", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached journal namespace should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_whitespace_journal_identifier_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.journal = Some(JournalConfig {
            namespace: "ops".to_string(),
            stderr: true,
            identifier: Some(" id ".to_string()),
        });

        let path = cache_path("unsafe-journal-identifier");
        fs::create_dir_all(path.parent().expect("cache path parent")).expect("create cache dir");
        let entry = CacheEntry {
            version: CACHE_ENTRY_VERSION,
            fingerprint: fingerprint.clone(),
            manifest,
        };
        fs::write(
            &path,
            bincode::serialize(&entry).expect("serialize cache entry"),
        )
        .expect("write cache file");

        assert!(load("unsafe-journal-identifier", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached journal identifier should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_whitespace_reconcile_function_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: PathBuf::from("hooks/reconcile.rhai"),
            function: " reconcile ".to_string(),
        });

        let path = cache_path("unsafe-reconcile");
        fs::create_dir_all(path.parent().expect("cache path parent")).expect("create cache dir");
        let entry = CacheEntry {
            version: CACHE_ENTRY_VERSION,
            fingerprint: fingerprint.clone(),
            manifest,
        };
        fs::write(
            &path,
            bincode::serialize(&entry).expect("serialize cache entry"),
        )
        .expect("write cache file");

        assert!(load("unsafe-reconcile", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached reconcile function should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_whitespace_env_key_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest
            .env
            .insert(" CACHE_ENV_KEY".to_string(), "value".to_string());

        let path = cache_path("unsafe-env");
        fs::create_dir_all(path.parent().expect("cache path parent")).expect("create cache dir");
        let entry = CacheEntry {
            version: CACHE_ENTRY_VERSION,
            fingerprint: fingerprint.clone(),
            manifest,
        };
        fs::write(
            &path,
            bincode::serialize(&entry).expect("serialize cache entry"),
        )
        .expect("write cache file");

        assert!(load("unsafe-env", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached env key should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }
}
