use crate::arg_validation::{self, ArgViolation};
use crate::env_util;
use crate::env_validation::{self, EnvKeyViolation, EnvValueViolation};
use crate::journal_validation::{self, JournalIdentifierViolation, JournalNamespaceViolation};
use crate::manifest::Manifest;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
    validate_cached_command_path(&manifest.exec, "cached manifest exec path")?;

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

    let mut seen_env_remove = HashSet::with_capacity(manifest.env_remove.len());
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
        if !seen_env_remove.insert(normalized_key) {
            return Err(anyhow!(
                "cached manifest env_remove keys cannot contain duplicates"
            ));
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
        validate_cached_command_path(&reconcile.script, "cached manifest reconcile script path")?;

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

fn validate_cached_command_path(path: &Path, field: &str) -> Result<()> {
    if path_contains_nul(path) {
        return Err(anyhow!("{field} cannot contain NUL bytes"));
    }

    let value = path.to_string_lossy();
    if value.is_empty() {
        return Err(anyhow!("{field} cannot be empty"));
    }
    if value.trim() != value {
        return Err(anyhow!("{field} cannot include surrounding whitespace"));
    }
    if value == "." || value == ".." {
        return Err(anyhow!("{field} cannot be `.` or `..`"));
    }
    if value.ends_with('/') || value.ends_with('\\') {
        return Err(anyhow!("{field} cannot end with a path separator"));
    }
    if ends_with_dot_component(&value) {
        return Err(anyhow!(
            "{field} cannot end with `.` or `..` path components"
        ));
    }
    Ok(())
}

fn ends_with_dot_component(value: &str) -> bool {
    let trimmed = value.trim_end_matches(['/', '\\']);
    matches!(trimmed.rsplit(['/', '\\']).next(), Some(".") | Some(".."))
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
    validate_cached_manifest(manifest)
        .context("refusing to store invalid alias cache entry manifest")?;

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
    fn store_rejects_invalid_manifest_and_skips_cache_write() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut invalid_manifest = Manifest::simple(PathBuf::from("echo"));
        invalid_manifest
            .env
            .insert("BAD=KEY".into(), "value".into());

        let err = store("invalid-store", &fingerprint, &invalid_manifest)
            .expect_err("invalid manifest should be rejected on store");
        assert!(
            err.to_string()
                .contains("refusing to store invalid alias cache entry manifest"),
            "{err}"
        );

        let path = cache_path("invalid-store");
        assert!(
            !path.exists(),
            "invalid manifest should not produce cache file"
        );
        env::remove_var("XDG_CACHE_HOME");
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
    fn cached_manifest_with_nul_exec_path_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let manifest = Manifest::simple(PathBuf::from("ec\0ho"));

        let path = cache_path("unsafe-nul-exec-path");
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

        assert!(load("unsafe-nul-exec-path", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached NUL exec path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_trailing_separator_exec_path_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let manifest = Manifest::simple(PathBuf::from("ech/"));

        let path = cache_path("unsafe-exec-path");
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

        assert!(load("unsafe-exec-path", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached exec path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_trailing_backslash_exec_path_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let manifest = Manifest::simple(PathBuf::from("ech\\"));

        let path = cache_path("unsafe-exec-path-backslash");
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

        assert!(load("unsafe-exec-path-backslash", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached backslash exec path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_dot_component_exec_path_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let manifest = Manifest::simple(PathBuf::from("/bin/.."));

        let path = cache_path("unsafe-exec-dot-component");
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

        assert!(load("unsafe-exec-dot-component", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached exec dot-component path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_dot_token_exec_path_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let manifest = Manifest::simple(PathBuf::from("."));

        let path = cache_path("unsafe-exec-dot-token");
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

        assert!(load("unsafe-exec-dot-token", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached exec dot-token path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_empty_exec_path_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let manifest = Manifest::simple(PathBuf::from(""));

        let path = cache_path("unsafe-empty-exec-path");
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

        assert!(load("unsafe-empty-exec-path", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached empty exec path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_whitespace_exec_path_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let manifest = Manifest::simple(PathBuf::from(" echo"));

        let path = cache_path("unsafe-whitespace-exec-path");
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

        assert!(load("unsafe-whitespace-exec-path", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached whitespace exec path should be pruned on load"
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
    fn cached_manifest_with_blank_journal_namespace_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.journal = Some(JournalConfig {
            namespace: "   ".to_string(),
            stderr: true,
            identifier: None,
        });

        let path = cache_path("unsafe-journal-namespace-blank");
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

        assert!(load("unsafe-journal-namespace-blank", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached blank journal namespace should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_nul_journal_namespace_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.journal = Some(JournalConfig {
            namespace: "ops\0prod".to_string(),
            stderr: true,
            identifier: None,
        });

        let path = cache_path("unsafe-journal-namespace-nul");
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

        assert!(load("unsafe-journal-namespace-nul", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached NUL journal namespace should be pruned on load"
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
    fn cached_manifest_with_nul_journal_identifier_is_pruned() {
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
            identifier: Some("id\0value".to_string()),
        });

        let path = cache_path("unsafe-journal-identifier-nul");
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

        assert!(load("unsafe-journal-identifier-nul", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached NUL journal identifier should be pruned on load"
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
    fn cached_manifest_with_nul_reconcile_function_is_pruned() {
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
            function: "reco\0ncile".to_string(),
        });

        let path = cache_path("unsafe-reconcile-function-nul");
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

        assert!(load("unsafe-reconcile-function-nul", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached NUL reconcile function should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_trailing_separator_reconcile_script_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: PathBuf::from("hooks/reconcile.rha/"),
            function: "reconcile".to_string(),
        });

        let path = cache_path("unsafe-reconcile-script-path");
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

        assert!(load("unsafe-reconcile-script-path", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached reconcile script path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_trailing_backslash_reconcile_script_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: PathBuf::from("hooks/reconcile.rha\\"),
            function: "reconcile".to_string(),
        });

        let path = cache_path("unsafe-reconcile-script-backslash");
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

        assert!(load("unsafe-reconcile-script-backslash", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached backslash reconcile script path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_dot_component_reconcile_script_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: PathBuf::from("hooks/s1/.."),
            function: "reconcile".to_string(),
        });

        let path = cache_path("unsafe-reconcile-script-dot-component");
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

        assert!(load("unsafe-reconcile-script-dot-component", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached reconcile script dot-component path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_dot_token_reconcile_script_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: PathBuf::from(".."),
            function: "reconcile".to_string(),
        });

        let path = cache_path("unsafe-reconcile-script-dot-token");
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

        assert!(load("unsafe-reconcile-script-dot-token", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached reconcile script dot-token path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_nul_reconcile_script_path_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: PathBuf::from("hooks/recon\0.rhai"),
            function: "reconcile".to_string(),
        });

        let path = cache_path("unsafe-nul-reconcile-script-path");
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

        assert!(load("unsafe-nul-reconcile-script-path", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached NUL reconcile script path should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_whitespace_reconcile_script_path_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: PathBuf::from(" hooks/reconcile.rhai"),
            function: "reconcile".to_string(),
        });

        let path = cache_path("unsafe-whitespace-reconcile-script-path");
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

        assert!(load("unsafe-whitespace-reconcile-script-path", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached whitespace reconcile script path should be pruned on load"
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

    #[test]
    fn cached_manifest_with_equals_env_key_is_pruned() {
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
            .insert("CACHE=KEY".to_string(), "value".to_string());

        let path = cache_path("unsafe-env-equals");
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

        assert!(load("unsafe-env-equals", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached env key containing '=' should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_nul_env_value_is_pruned() {
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
            .insert("CACHE_KEY".to_string(), "bad\0value".to_string());

        let path = cache_path("unsafe-env-nul-value");
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

        assert!(load("unsafe-env-nul-value", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached env value containing NUL should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_nul_env_key_is_pruned() {
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
            .insert("CACHE\0KEY".to_string(), "value".to_string());

        let path = cache_path("unsafe-env-nul-key");
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

        assert!(load("unsafe-env-nul-key", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached env key containing NUL should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_whitespace_env_remove_key_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.env_remove = vec![" CACHE_REMOVE_KEY".to_string()];

        let path = cache_path("unsafe-env-remove");
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

        assert!(load("unsafe-env-remove", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached env_remove key should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_duplicate_env_remove_keys_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.env_remove = vec!["CACHE_DUP_KEY".to_string(), "CACHE_DUP_KEY".to_string()];

        let path = cache_path("unsafe-duplicate-env-remove");
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

        assert!(load("unsafe-duplicate-env-remove", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached duplicate env_remove keys should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_equals_env_remove_key_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.env_remove = vec!["CACHE=REMOVE".to_string()];

        let path = cache_path("unsafe-env-remove-equals");
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

        assert!(load("unsafe-env-remove-equals", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached env_remove key containing '=' should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn cached_manifest_with_nul_env_remove_key_is_pruned() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let home = TempDir::new().expect("create tempdir");
        env::set_var("XDG_CACHE_HOME", home.path());

        let config_dir = TempDir::new().expect("create config dir");
        let source_file = config_dir.path().join("a.toml");
        fs::write(&source_file, "exec = \"echo\"\n").expect("write source");
        let fingerprint = source_fingerprint(&source_file).expect("source fingerprint");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.env_remove = vec!["CACHE\0REMOVE".to_string()];

        let path = cache_path("unsafe-env-remove-nul");
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

        assert!(load("unsafe-env-remove-nul", &fingerprint).is_none());
        assert!(
            !path.exists(),
            "invalid cached env_remove key containing NUL should be pruned on load"
        );
        env::remove_var("XDG_CACHE_HOME");
    }
}
