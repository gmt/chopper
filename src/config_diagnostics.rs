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

fn path_is_explicit(path: &Path) -> bool {
    path.is_absolute() || path.components().count() > 1
}

#[cfg(test)]
mod tests {
    use super::{manifest_missing_target_warnings, scan_extension_warnings};
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
}
