use crate::env_util;
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
    let cache_path = cache_path(alias)?;
    let bytes = fs::read(&cache_path).ok()?;
    let entry: CacheEntry = match bincode::deserialize(&bytes) {
        Ok(entry) => entry,
        Err(_) => {
            delete_cache_file_best_effort(&cache_path);
            return None;
        }
    };
    if entry.version != CACHE_ENTRY_VERSION {
        delete_cache_file_best_effort(&cache_path);
        return None;
    }
    if entry.fingerprint == *fingerprint {
        Some(entry.manifest)
    } else {
        delete_cache_file_best_effort(&cache_path);
        None
    }
}

pub fn store(alias: &str, fingerprint: &SourceFingerprint, manifest: &Manifest) -> Result<()> {
    let path = cache_path(alias)
        .ok_or_else(|| anyhow::anyhow!("failed to compute cache path for alias `{alias}`"))?;
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

fn cache_path(alias: &str) -> Option<PathBuf> {
    let cache_dir = cache_dir();

    let safe_alias = alias
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | ' ' => '_',
            other => other,
        })
        .collect::<String>();

    Some(
        cache_dir
            .join("manifests")
            .join(format!("{safe_alias}.bin")),
    )
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
        cache_path, cache_temp_path, load, source_fingerprint, store, CacheEntry,
        CACHE_ENTRY_VERSION,
    };
    use crate::manifest::Manifest;
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
        let cache_file = cache_path("demo").expect("cache path");
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

        let path = cache_path("demo").expect("cache path").to_path_buf();
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

        let path = cache_path("demo").expect("cache path");
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
}
