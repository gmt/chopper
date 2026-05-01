use crate::manifest::Manifest;
use crate::rhai_wiring;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

pub(crate) fn scan_extension_warnings(config_root: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    collect_extension_warnings(&config_root.join("aliases"), &mut warnings);
    collect_extension_warnings(config_root, &mut warnings);
    collect_canonical_alias_dir_extension_warnings(config_root, &mut warnings);
    warnings.sort();
    warnings.dedup();
    warnings
}

pub(crate) fn scan_bashcomp_file_warnings(config_root: &Path) -> Vec<String> {
    let aliases = collect_alias_names(config_root);
    let dirs = bash_completion_dirs();
    let mut warnings = Vec::new();

    for alias in aliases {
        for dir in &dirs {
            for candidate in bash_completion_candidates(dir, &alias) {
                if let Some(warning) = bash_completion_file_warning(&alias, &candidate) {
                    warnings.push(warning);
                }
            }
        }
    }

    warnings.sort();
    warnings.dedup();
    warnings
}

fn collect_extension_warnings(dir: &Path, warnings: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            warnings.push(format!("could not scan {}: {err}", dir.display()));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warnings.push(format!("could not read entry in {}: {err}", dir.display()));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(extension) = Path::new(file_name)
            .extension()
            .and_then(|ext| ext.to_str())
        else {
            continue;
        };
        if extension.eq_ignore_ascii_case("toml") || extension.eq_ignore_ascii_case("rhai") {
            continue;
        }
        warnings.push(format!(
            "suspicious config file extension (expected .toml/.rhai): {}",
            path.display()
        ));
    }
}

fn collect_canonical_alias_dir_extension_warnings(config_root: &Path, warnings: &mut Vec<String>) {
    let entries = match std::fs::read_dir(config_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            warnings.push(format!("could not scan {}: {err}", config_root.display()));
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if entry.file_name() == "aliases" {
            continue;
        }
        collect_extension_warnings(&path, warnings);
    }
}

pub(crate) fn manifest_missing_target_warnings(manifest: &Manifest) -> Vec<String> {
    let mut warnings = Vec::new();
    if path_is_explicit(&manifest.exec) && !manifest.exec.exists() {
        warnings.push(format!(
            "exec target does not exist: {}",
            manifest.exec.display()
        ));
    }

    if let Some(reconcile) = &manifest.reconcile {
        if !reconcile.script.exists() {
            warnings.push(format!(
                "reconcile script does not exist: {}",
                reconcile.script.display()
            ));
        }
    }

    if let Some(bashcomp) = &manifest.bashcomp {
        if let Some(script) = &bashcomp.script {
            if !script.exists() {
                warnings.push(format!(
                    "bash completion script does not exist: {}",
                    script.display()
                ));
            }
        }
        if let Some(rhai_script) = &bashcomp.rhai_script {
            if !rhai_script.exists() {
                warnings.push(format!(
                    "bash completion Rhai script does not exist: {}",
                    rhai_script.display()
                ));
            } else if let Some(function) = &bashcomp.rhai_function {
                match rhai_wiring::read_compatible_methods(rhai_script) {
                    Ok(methods) => {
                        if !methods.iter().any(|method| method == function) {
                            warnings.push(format!(
                                "bash completion Rhai function `{}` does not exist in {}",
                                function,
                                rhai_script.display()
                            ));
                        }
                    }
                    Err(err) => warnings.push(format!(
                        "bash completion Rhai script could not be inspected: {}: {err}",
                        rhai_script.display()
                    )),
                }
            }
        }
    }
    warnings
}

fn collect_alias_names(config_root: &Path) -> BTreeSet<String> {
    let mut aliases: BTreeSet<String> = crate::alias_paths::discover_exec_aliases(config_root)
        .unwrap_or_default()
        .into_iter()
        .collect();
    collect_alias_names_from_dir(&config_root.join("aliases"), &mut aliases);
    collect_alias_names_from_dir(config_root, &mut aliases);
    aliases
}

fn collect_alias_names_from_dir(dir: &Path, aliases: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(alias) = file_name.strip_suffix(".toml") else {
            continue;
        };
        if !alias.is_empty() {
            aliases.insert(alias.to_string());
        }
    }
}

fn bash_completion_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(user_dirs) = std::env::var_os("BASH_COMPLETION_USER_DIR") {
        for dir in std::env::split_paths(&user_dirs) {
            dirs.push(dir.join("completions"));
        }
    } else if let Some(xdg_data_home) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(std::path::PathBuf::from(xdg_data_home).join("bash-completion/completions"));
    } else if let Some(home) = std::env::var_os("HOME") {
        dirs.push(std::path::PathBuf::from(home).join(".local/share/bash-completion/completions"));
    }

    if let Some(xdg_data_dirs) = std::env::var_os("XDG_DATA_DIRS") {
        for dir in std::env::split_paths(&xdg_data_dirs) {
            dirs.push(dir.join("bash-completion/completions"));
        }
    }

    dirs.push(std::path::PathBuf::from(
        "/usr/local/share/bash-completion/completions",
    ));
    dirs.push(std::path::PathBuf::from(
        "/usr/share/bash-completion/completions",
    ));
    dirs.push(std::path::PathBuf::from("/etc/bash_completion.d"));

    dirs
}

fn bash_completion_candidates(dir: &Path, alias: &str) -> [std::path::PathBuf; 3] {
    [
        dir.join(alias),
        dir.join(format!("{alias}.bash")),
        dir.join(format!("_{alias}")),
    ]
}

fn bash_completion_file_warning(alias: &str, path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }

    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    file.by_ref().take(8192).read_to_end(&mut buf).ok()?;
    if buf.is_empty() {
        return Some(format!(
            "bash completion file for alias `{}` is empty and may shadow the real completer: {}",
            alias,
            path.display()
        ));
    }

    let content = String::from_utf8_lossy(&buf);
    if content.trim().is_empty() {
        return Some(format!(
            "bash completion file for alias `{}` is whitespace-only and may shadow the real completer: {}",
            alias,
            path.display()
        ));
    }

    let first_line = content.lines().next().unwrap_or("");
    if first_line.starts_with("# Auto-generated by chopper") {
        return None;
    }

    if first_line == "# Bash completion for chopper-managed aliases."
        || content.contains("chopper --bashcomp")
        || content.contains("_chopper_complete")
    {
        return Some(format!(
            "bash completion file for alias `{}` appears to point back to chopper completion and may shadow the real completer: {}",
            alias,
            path.display()
        ));
    }

    None
}

fn path_is_explicit(path: &Path) -> bool {
    path.is_absolute() || path.components().count() > 1
}

#[cfg(test)]
mod tests {
    use super::{
        manifest_missing_target_warnings, scan_bashcomp_file_warnings, scan_extension_warnings,
    };
    use crate::manifest::{BashcompConfig, Manifest, ReconcileConfig};
    use crate::test_support::ENV_LOCK;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn restore_env_var(key: &str, value: Option<std::ffi::OsString>) {
        if let Some(value) = value {
            env::set_var(key, value);
        } else {
            env::remove_var(key);
        }
    }

    #[test]
    fn scan_extension_warnings_flags_unknown_extensions_only() {
        let temp = TempDir::new().expect("tempdir");
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir).expect("create aliases dir");
        fs::write(aliases_dir.join("good.toml"), "exec = \"echo\"\n").expect("write toml");
        fs::write(aliases_dir.join("good.rhai"), "fn reconcile(ctx) { #{} }\n")
            .expect("write rhai");
        fs::write(aliases_dir.join("odd.ini"), "value=1\n").expect("write ini");

        let warnings = scan_extension_warnings(temp.path());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("odd.ini"), "{warnings:?}");
    }

    #[test]
    fn manifest_missing_target_warnings_only_checks_explicit_paths() {
        let mut manifest = Manifest::simple(PathBuf::from("missing-binary-name"));
        manifest.reconcile = Some(ReconcileConfig {
            script: PathBuf::from("/definitely/missing/script.rhai"),
            function: String::from("reconcile"),
        });
        manifest.bashcomp = Some(BashcompConfig {
            disabled: false,
            passthrough: false,
            script: Some(PathBuf::from("/definitely/missing/comp.bash")),
            rhai_script: Some(PathBuf::from("/definitely/missing/comp.rhai")),
            rhai_function: Some(String::from("complete")),
        });

        let warnings = manifest_missing_target_warnings(&manifest);
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(
            warnings
                .iter()
                .all(|warning| !warning.contains("missing-binary-name")),
            "{warnings:?}"
        );
    }

    #[test]
    fn scan_bashcomp_file_warnings_flags_empty_user_completion_shadow() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = TempDir::new().expect("tempdir");
        let config_root = temp.path().join("chopper");
        let aliases_dir = config_root.join("aliases");
        fs::create_dir_all(&aliases_dir).expect("create aliases dir");
        fs::write(aliases_dir.join("demo.toml"), "exec = \"echo\"\n").expect("write alias");

        let completion_base = temp.path().join("bash-completion-user");
        let completion_dir = completion_base.join("completions");
        fs::create_dir_all(&completion_dir).expect("create completion dir");
        fs::write(completion_dir.join("demo"), "").expect("write empty completion");

        let old_user_dir = env::var_os("BASH_COMPLETION_USER_DIR");
        let old_xdg_data_home = env::var_os("XDG_DATA_HOME");
        let old_xdg_data_dirs = env::var_os("XDG_DATA_DIRS");
        let old_home = env::var_os("HOME");
        env::set_var("BASH_COMPLETION_USER_DIR", &completion_base);
        env::remove_var("XDG_DATA_HOME");
        env::remove_var("XDG_DATA_DIRS");
        env::set_var("HOME", temp.path());

        let warnings = scan_bashcomp_file_warnings(&config_root);

        restore_env_var("BASH_COMPLETION_USER_DIR", old_user_dir);
        restore_env_var("XDG_DATA_HOME", old_xdg_data_home);
        restore_env_var("XDG_DATA_DIRS", old_xdg_data_dirs);
        restore_env_var("HOME", old_home);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("is empty"), "{warnings:?}");
        assert!(warnings[0].contains("demo"), "{warnings:?}");
    }

    #[test]
    fn scan_bashcomp_file_warnings_flags_chopper_self_reference() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = TempDir::new().expect("tempdir");
        let config_root = temp.path().join("chopper");
        let aliases_dir = config_root.join("aliases");
        fs::create_dir_all(&aliases_dir).expect("create aliases dir");
        fs::write(aliases_dir.join("demo.toml"), "exec = \"echo\"\n").expect("write alias");

        let completion_base = temp.path().join("bash-completion-user");
        let completion_dir = completion_base.join("completions");
        fs::create_dir_all(&completion_dir).expect("create completion dir");
        fs::write(
            completion_dir.join("demo"),
            "# Bash completion for chopper-managed aliases.\n_chopper_complete() { :; }\n",
        )
        .expect("write self-referential completion");

        let old_user_dir = env::var_os("BASH_COMPLETION_USER_DIR");
        let old_xdg_data_home = env::var_os("XDG_DATA_HOME");
        let old_xdg_data_dirs = env::var_os("XDG_DATA_DIRS");
        let old_home = env::var_os("HOME");
        env::set_var("BASH_COMPLETION_USER_DIR", &completion_base);
        env::remove_var("XDG_DATA_HOME");
        env::remove_var("XDG_DATA_DIRS");
        env::set_var("HOME", temp.path());

        let warnings = scan_bashcomp_file_warnings(&config_root);

        restore_env_var("BASH_COMPLETION_USER_DIR", old_user_dir);
        restore_env_var("XDG_DATA_HOME", old_xdg_data_home);
        restore_env_var("XDG_DATA_DIRS", old_xdg_data_dirs);
        restore_env_var("HOME", old_home);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains("point back to chopper"),
            "{warnings:?}"
        );
    }

    #[test]
    fn manifest_missing_target_warnings_flags_missing_bashcomp_rhai_function() {
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("demo.rhai");
        fs::write(&script_path, "fn other(ctx) { [] }\n").expect("write rhai");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.bashcomp = Some(BashcompConfig {
            disabled: false,
            passthrough: false,
            script: None,
            rhai_script: Some(script_path),
            rhai_function: Some(String::from("complete")),
        });

        let warnings = manifest_missing_target_warnings(&manifest);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains("Rhai function `complete` does not exist"),
            "{warnings:?}"
        );
    }
}
