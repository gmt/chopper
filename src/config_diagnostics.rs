use crate::manifest::Manifest;
use std::path::Path;

pub(crate) fn scan_extension_warnings(config_root: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    collect_extension_warnings(&config_root.join("aliases"), &mut warnings);
    collect_extension_warnings(config_root, &mut warnings);
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
            }
        }
    }
    warnings
}

pub(crate) fn scan_legacy_script_field_warnings(config_root: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    collect_legacy_script_warnings(&config_root.join("aliases"), &mut warnings);
    collect_legacy_script_warnings(config_root, &mut warnings);
    warnings.sort();
    warnings.dedup();
    warnings
}

pub(crate) fn legacy_script_field_warnings_for_path(path: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    append_legacy_script_warnings_for_file(path, &mut warnings);
    warnings
}

fn path_is_explicit(path: &Path) -> bool {
    path.is_absolute() || path.components().count() > 1
}

fn collect_legacy_script_warnings(dir: &Path, warnings: &mut Vec<String>) {
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
        let is_toml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("toml"))
            .unwrap_or(false);
        if !is_toml {
            continue;
        }
        append_legacy_script_warnings_for_file(&path, warnings);
    }
}

fn append_legacy_script_warnings_for_file(path: &Path, warnings: &mut Vec<String>) {
    let doc = match crate::alias_doc::load_alias_doc(path) {
        Ok(doc) => doc,
        Err(_) => return,
    };
    let has_legacy_reconcile_script = doc
        .reconcile
        .as_ref()
        .and_then(|reconcile| reconcile.script.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    if has_legacy_reconcile_script {
        warnings.push(format!(
            "legacy field `reconcile.script` is ignored: {}",
            path.display()
        ));
    }
    let has_legacy_bashcomp_rhai_script = doc
        .bashcomp
        .as_ref()
        .and_then(|bashcomp| bashcomp.rhai_script.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    if has_legacy_bashcomp_rhai_script {
        warnings.push(format!(
            "legacy field `bashcomp.rhai_script` is ignored: {}",
            path.display()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        legacy_script_field_warnings_for_path, manifest_missing_target_warnings,
        scan_extension_warnings, scan_legacy_script_field_warnings,
    };
    use crate::manifest::{BashcompConfig, Manifest, ReconcileConfig};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

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
    fn scan_legacy_script_field_warnings_flags_ignored_fields() {
        let temp = TempDir::new().expect("tempdir");
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir).expect("create aliases dir");
        fs::write(
            aliases_dir.join("legacy.toml"),
            r#"
exec = "echo"
[reconcile]
script = "legacy.rhai"
function = "reconcile"
[bashcomp]
rhai_script = "legacy-complete.rhai"
rhai_function = "complete"
"#,
        )
        .expect("write legacy file");

        let warnings = scan_legacy_script_field_warnings(temp.path());
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("reconcile.script")));
        assert!(warnings.iter().any(|w| w.contains("bashcomp.rhai_script")));
    }

    #[test]
    fn legacy_script_field_warnings_for_path_reports_single_alias_file() {
        let temp = TempDir::new().expect("tempdir");
        let alias_path = temp.path().join("demo.toml");
        fs::write(
            &alias_path,
            r#"
exec = "echo"
[reconcile]
script = "legacy.rhai"
function = "reconcile"
"#,
        )
        .expect("write alias");
        let warnings = legacy_script_field_warnings_for_path(&alias_path);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("reconcile.script"));
    }
}
