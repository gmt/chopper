use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::symlink;

const EXEC_ALIAS_FILE: &str = "exe.toml";
const LEGACY_ALIASES_DIR: &str = "aliases";

pub(crate) fn default_exec_config_path(config_root: &Path, alias: &str) -> PathBuf {
    config_root.join(alias).join(EXEC_ALIAS_FILE)
}

fn exec_config_candidates(config_root: &Path, alias: &str) -> [PathBuf; 3] {
    [
        default_exec_config_path(config_root, alias),
        config_root
            .join(LEGACY_ALIASES_DIR)
            .join(format!("{alias}.toml")),
        config_root.join(format!("{alias}.toml")),
    ]
}

pub fn find_exec_config(config_root: &Path, alias: &str) -> Option<PathBuf> {
    let _ = auto_upgrade_exec_config(config_root, alias);
    exec_config_candidates(config_root, alias)
        .into_iter()
        .find(|path| path.is_file())
}

pub fn discover_exec_aliases(config_root: &Path) -> Result<Vec<String>> {
    let _ = auto_upgrade_exec_configs(config_root);
    let mut aliases = BTreeSet::new();
    discover_canonical_exec_aliases(config_root, &mut aliases)?;
    discover_legacy_aliases_in_dir(&config_root.join(LEGACY_ALIASES_DIR), &mut aliases)?;
    discover_legacy_aliases_in_dir(config_root, &mut aliases)?;
    Ok(aliases.into_iter().collect())
}

pub(crate) fn target_path_like_source(
    config_root: &Path,
    source_path: &Path,
    source_alias: &str,
    target_alias: &str,
) -> PathBuf {
    if is_default_exec_config_path(config_root, source_path, source_alias) {
        return default_exec_config_path(config_root, target_alias);
    }

    let legacy_aliases_path = config_root
        .join(LEGACY_ALIASES_DIR)
        .join(format!("{source_alias}.toml"));
    if paths_equal(source_path, &legacy_aliases_path) {
        return config_root
            .join(LEGACY_ALIASES_DIR)
            .join(format!("{target_alias}.toml"));
    }

    let legacy_root_path = config_root.join(format!("{source_alias}.toml"));
    if paths_equal(source_path, &legacy_root_path) {
        return config_root.join(format!("{target_alias}.toml"));
    }

    default_exec_config_path(config_root, target_alias)
}

pub(crate) fn try_remove_empty_alias_dir(config_root: &Path, alias: &str, config_path: &Path) {
    if !is_default_exec_config_path(config_root, config_path, alias) {
        return;
    }
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

fn auto_upgrade_exec_configs(config_root: &Path) -> Result<()> {
    let mut aliases = BTreeSet::new();
    discover_legacy_aliases_in_dir(&config_root.join(LEGACY_ALIASES_DIR), &mut aliases)?;
    discover_legacy_aliases_in_dir(config_root, &mut aliases)?;

    for alias in aliases {
        let _ = auto_upgrade_exec_config(config_root, &alias);
    }
    Ok(())
}

fn auto_upgrade_exec_config(config_root: &Path, alias: &str) -> Result<Option<PathBuf>> {
    let canonical = default_exec_config_path(config_root, alias);
    if canonical.is_file() {
        return Ok(Some(canonical));
    }

    let candidates = exec_config_candidates(config_root, alias);
    for legacy in candidates.iter().skip(1) {
        if !legacy.is_file() {
            continue;
        }
        upgrade_legacy_exec_config(legacy, &canonical)?;
        return Ok(Some(canonical));
    }

    Ok(None)
}

fn upgrade_legacy_exec_config(legacy: &Path, canonical: &Path) -> Result<()> {
    if let Some(parent) = canonical.parent() {
        fs_err::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let metadata = fs_err::symlink_metadata(legacy)
        .with_context(|| format!("failed to inspect legacy alias {}", legacy.display()))?;
    if metadata.file_type().is_symlink() {
        upgrade_symlinked_legacy_exec_config(legacy, canonical)?;
    } else {
        upgrade_regular_legacy_exec_config(legacy, canonical)?;
    }
    Ok(())
}

#[cfg(unix)]
fn upgrade_symlinked_legacy_exec_config(legacy: &Path, canonical: &Path) -> Result<()> {
    let target = fs_err::canonicalize(legacy).with_context(|| {
        format!(
            "failed to resolve legacy alias symlink {}",
            legacy.display()
        )
    })?;
    symlink(&target, canonical).with_context(|| {
        format!(
            "failed to create canonical alias symlink {} -> {}",
            canonical.display(),
            target.display()
        )
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn upgrade_symlinked_legacy_exec_config(legacy: &Path, canonical: &Path) -> Result<()> {
    let target = fs_err::canonicalize(legacy).with_context(|| {
        format!(
            "failed to resolve legacy alias symlink {}",
            legacy.display()
        )
    })?;
    fs_err::copy(&target, canonical).with_context(|| {
        format!(
            "failed to copy legacy alias symlink target {} to {}",
            target.display(),
            canonical.display()
        )
    })?;
    Ok(())
}

fn upgrade_regular_legacy_exec_config(legacy: &Path, canonical: &Path) -> Result<()> {
    let target = fs_err::canonicalize(legacy)
        .with_context(|| format!("failed to resolve legacy alias {}", legacy.display()))?;
    create_canonical_legacy_link(&target, canonical).with_context(|| {
        format!(
            "failed to create canonical alias symlink {} -> {}",
            canonical.display(),
            target.display()
        )
    })?;
    Ok(())
}

#[cfg(unix)]
fn create_canonical_legacy_link(target: &Path, canonical: &Path) -> Result<()> {
    symlink(target, canonical).with_context(|| {
        format!(
            "failed to create canonical alias symlink {} -> {}",
            canonical.display(),
            target.display()
        )
    })
}

#[cfg(not(unix))]
fn create_canonical_legacy_link(target: &Path, canonical: &Path) -> Result<()> {
    fs_err::copy(target, canonical).with_context(|| {
        format!(
            "failed to copy legacy alias {} to {}",
            target.display(),
            canonical.display()
        )
    })?;
    Ok(())
}

fn discover_canonical_exec_aliases(
    config_root: &Path,
    aliases: &mut BTreeSet<String>,
) -> Result<()> {
    let entries = match std::fs::read_dir(config_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", config_root.display()))
        }
    };

    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read {}", config_root.display()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if entry.file_name() == LEGACY_ALIASES_DIR {
            continue;
        }
        if !path.join(EXEC_ALIAS_FILE).is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(alias) = name.to_str() else {
            continue;
        };
        if !alias.is_empty() {
            aliases.insert(alias.to_string());
        }
    }
    Ok(())
}

fn discover_legacy_aliases_in_dir(dir: &Path, aliases: &mut BTreeSet<String>) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", dir.display())),
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(alias) = file_name.strip_suffix(".toml") else {
            continue;
        };
        if !alias.is_empty() {
            aliases.insert(alias.to_string());
        }
    }
    Ok(())
}

fn is_default_exec_config_path(config_root: &Path, path: &Path, alias: &str) -> bool {
    paths_equal(path, &default_exec_config_path(config_root, alias))
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    left == right
}

#[cfg(test)]
mod tests {
    use super::find_exec_config;
    use std::fs;
    use tempfile::TempDir;

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    #[test]
    fn find_exec_config_upgrades_legacy_aliases_dir_file() {
        let temp = TempDir::new().expect("tempdir");
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(aliases_dir.join("bin")).expect("create aliases bin dir");
        let legacy = aliases_dir.join("demo.toml");
        fs::write(&legacy, "exec = \"bin/run\"\n").expect("write legacy alias");
        fs::write(aliases_dir.join("demo.rhai"), "fn reconcile(ctx) { #{} }\n")
            .expect("write shared rhai");

        let found = find_exec_config(temp.path(), "demo").expect("find upgraded config");
        let canonical = temp.path().join("demo/exe.toml");
        assert_eq!(found, canonical);
        assert!(canonical.is_file());
        assert!(legacy.is_file());
        assert!(aliases_dir.join("demo.rhai").is_file());
        assert_eq!(
            fs::canonicalize(&canonical).expect("canonical symlink target"),
            legacy
        );
    }

    #[test]
    fn find_exec_config_upgrades_legacy_root_file() {
        let temp = TempDir::new().expect("tempdir");
        fs::write(temp.path().join("rooty.toml"), "exec = \"bin/rooty\"\n")
            .expect("write root legacy alias");

        let found = find_exec_config(temp.path(), "rooty").expect("find upgraded config");
        let canonical = temp.path().join("rooty/exe.toml");
        assert_eq!(found, canonical);
        assert!(canonical.is_file());
        assert!(temp.path().join("rooty.toml").is_file());
        assert_eq!(
            fs::canonicalize(&canonical).expect("canonical symlink target"),
            temp.path().join("rooty.toml")
        );
    }

    #[cfg(unix)]
    #[test]
    fn find_exec_config_upgrades_legacy_symlink_to_canonical_symlink() {
        let temp = TempDir::new().expect("tempdir");
        let aliases_dir = temp.path().join("aliases");
        let shared_dir = temp.path().join("shared");
        fs::create_dir_all(&aliases_dir).expect("create aliases dir");
        fs::create_dir_all(&shared_dir).expect("create shared dir");
        let target = shared_dir.join("target.toml");
        fs::write(&target, "exec = \"bin/run\"\n").expect("write target");
        symlink("../shared/target.toml", aliases_dir.join("linked.toml"))
            .expect("create relative legacy symlink");

        let found = find_exec_config(temp.path(), "linked").expect("find upgraded config");
        let canonical = temp.path().join("linked/exe.toml");
        assert_eq!(found, canonical);
        assert!(canonical.is_file());
        assert!(aliases_dir.join("linked.toml").is_file());
        assert_eq!(
            fs::canonicalize(&canonical).expect("canonical symlink target"),
            target
        );
    }
}
