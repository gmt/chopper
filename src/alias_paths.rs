use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

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
    exec_config_candidates(config_root, alias)
        .into_iter()
        .find(|path| path.is_file())
}

pub fn discover_exec_aliases(config_root: &Path) -> Result<Vec<String>> {
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
