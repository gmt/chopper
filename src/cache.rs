use crate::manifest::Manifest;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

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
    let bytes = fs::read(cache_path).ok()?;
    let entry: CacheEntry = bincode::deserialize(&bytes).ok()?;
    if entry.fingerprint == *fingerprint {
        Some(entry.manifest)
    } else {
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
        fingerprint: fingerprint.clone(),
        manifest: manifest.clone(),
    };
    let bytes = bincode::serialize(&entry).context("failed to serialize alias cache entry")?;

    let tmp_path = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp_path, bytes).with_context(|| {
        format!(
            "failed to write temporary alias cache file {}",
            tmp_path.display()
        )
    })?;
    fs::rename(&tmp_path, &path).with_context(|| {
        format!(
            "failed to finalize alias cache file {}",
            path.as_path().display()
        )
    })?;
    Ok(())
}

fn cache_path(alias: &str) -> Option<PathBuf> {
    let cache_dir = cache_root_dir();

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

fn cache_root_dir() -> PathBuf {
    if let Ok(override_dir) = env::var("CHOPPER_CACHE_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    directories::ProjectDirs::from("", "", "chopper")
        .map(|d| d.cache_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".chopper-cache"))
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
    use super::{load, source_fingerprint, store};
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
}
