use crate::arg_validation::{self, ArgViolation};
use crate::env_validation::{self, EnvKeyViolation, EnvValueViolation};
use crate::journal_validation::{self, JournalIdentifierViolation, JournalNamespaceViolation};
use crate::manifest::{BashcompConfig, JournalConfig, Manifest, ReconcileConfig};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn parse(path: &Path) -> Result<Manifest> {
    if !is_toml_path(path) {
        return Err(anyhow!(
            "unsupported alias config format `{}`; expected a .toml file",
            path.display()
        ));
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read alias config {}", path.display()))?;
    parse_toml(strip_utf8_bom(&content), path)
}

fn is_toml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("toml"))
        .unwrap_or(false)
}

fn strip_utf8_bom(content: &str) -> &str {
    content.strip_prefix('\u{feff}').unwrap_or(content)
}

fn parse_toml(content: &str, path: &Path) -> Result<Manifest> {
    let parsed: AliasConfig =
        toml::from_str(content).with_context(|| format!("invalid TOML in {}", path.display()))?;
    let base_dir = config_base_dir(path);

    let exec = parsed.exec.trim();
    if exec.is_empty() {
        return Err(anyhow!("field `exec` cannot be empty"));
    }
    if exec.contains('\0') {
        return Err(anyhow!("field `exec` cannot contain NUL bytes"));
    }
    if exec == "." || exec == ".." {
        return Err(anyhow!("field `exec` cannot be `.` or `..`"));
    }
    if exec.ends_with('/') || exec.ends_with('\\') {
        return Err(anyhow!("field `exec` cannot end with a path separator"));
    }
    if ends_with_dot_component(exec) {
        return Err(anyhow!(
            "field `exec` cannot end with `.` or `..` path components"
        ));
    }
    if looks_like_relative_exec_path(exec) && !has_meaningful_relative_segment(exec) {
        return Err(anyhow!(
            "field `exec` must include a path segment when using relative path notation"
        ));
    }
    validate_arg_values(&parsed.args, "field `args`")?;

    let exec = resolve_exec_path(&base_dir, exec);

    let mut manifest = Manifest::simple(exec).with_args(parsed.args);
    manifest.env = normalize_env_map(parsed.env)?;
    manifest.env_remove = normalize_env_remove(parsed.env_remove)?;

    if let Some(journal) = parsed.journal {
        let namespace = match journal_validation::normalize_namespace(&journal.namespace) {
            Ok(namespace) => namespace,
            Err(JournalNamespaceViolation::Empty) => {
                return Err(anyhow!("field `journal.namespace` cannot be empty"));
            }
            Err(JournalNamespaceViolation::ContainsNul) => {
                return Err(anyhow!(
                    "field `journal.namespace` cannot contain NUL bytes"
                ));
            }
        };
        let identifier = match journal_validation::normalize_optional_identifier_for_config(
            journal.identifier.as_deref(),
        ) {
            Ok(identifier) => identifier,
            Err(JournalIdentifierViolation::ContainsNul) => {
                return Err(anyhow!(
                    "field `journal.identifier` cannot contain NUL bytes"
                ));
            }
            Err(JournalIdentifierViolation::Blank) => {
                unreachable!("blank identifiers are normalized to None for config parsing")
            }
        };
        manifest = manifest.with_journal(JournalConfig {
            namespace,
            stderr: journal.stderr,
            identifier,
        });
    }

    if let Some(reconcile) = parsed.reconcile {
        let script = reconcile.script.trim();
        if script.is_empty() {
            return Err(anyhow!("field `reconcile.script` cannot be empty"));
        }
        if script.contains('\0') {
            return Err(anyhow!("field `reconcile.script` cannot contain NUL bytes"));
        }
        if script == "." || script == ".." {
            return Err(anyhow!("field `reconcile.script` cannot be `.` or `..`"));
        }
        if script.ends_with('/') || script.ends_with('\\') {
            return Err(anyhow!(
                "field `reconcile.script` cannot end with a path separator"
            ));
        }
        if ends_with_dot_component(script) {
            return Err(anyhow!(
                "field `reconcile.script` cannot end with `.` or `..` path components"
            ));
        }
        if !Path::new(script).is_absolute() && !has_meaningful_relative_segment(script) {
            return Err(anyhow!(
                "field `reconcile.script` must include a file path when using relative path notation"
            ));
        }
        let script = resolve_script_path(&base_dir, script);
        let function = match reconcile.function {
            Some(f) => {
                let function = f.trim();
                if function.is_empty() {
                    "reconcile".to_string()
                } else if function.contains('\0') {
                    return Err(anyhow!(
                        "field `reconcile.function` cannot contain NUL bytes"
                    ));
                } else {
                    function.to_string()
                }
            }
            None => "reconcile".to_string(),
        };
        manifest = manifest.with_reconcile(ReconcileConfig { script, function });
    }

    if let Some(bashcomp) = parsed.bashcomp {
        let script = if let Some(script_str) = bashcomp.script {
            let script = script_str.trim();
            if script.is_empty() {
                None
            } else {
                if script.contains('\0') {
                    return Err(anyhow!("field `bashcomp.script` cannot contain NUL bytes"));
                }
                if script == "." || script == ".." {
                    return Err(anyhow!("field `bashcomp.script` cannot be `.` or `..`"));
                }
                if script.ends_with('/') || script.ends_with('\\') {
                    return Err(anyhow!(
                        "field `bashcomp.script` cannot end with a path separator"
                    ));
                }
                if ends_with_dot_component(script) {
                    return Err(anyhow!(
                        "field `bashcomp.script` cannot end with `.` or `..` path components"
                    ));
                }
                if !Path::new(script).is_absolute() && !has_meaningful_relative_segment(script) {
                    return Err(anyhow!(
                        "field `bashcomp.script` must include a file path when using relative path notation"
                    ));
                }
                Some(resolve_script_path(&base_dir, script))
            }
        } else {
            None
        };

        let rhai_script = if let Some(rhai_script_str) = bashcomp.rhai_script {
            let rhai_script = rhai_script_str.trim();
            if rhai_script.is_empty() {
                None
            } else {
                if rhai_script.contains('\0') {
                    return Err(anyhow!(
                        "field `bashcomp.rhai_script` cannot contain NUL bytes"
                    ));
                }
                if rhai_script == "." || rhai_script == ".." {
                    return Err(anyhow!(
                        "field `bashcomp.rhai_script` cannot be `.` or `..`"
                    ));
                }
                if rhai_script.ends_with('/') || rhai_script.ends_with('\\') {
                    return Err(anyhow!(
                        "field `bashcomp.rhai_script` cannot end with a path separator"
                    ));
                }
                if ends_with_dot_component(rhai_script) {
                    return Err(anyhow!(
                        "field `bashcomp.rhai_script` cannot end with `.` or `..` path components"
                    ));
                }
                if !Path::new(rhai_script).is_absolute()
                    && !has_meaningful_relative_segment(rhai_script)
                {
                    return Err(anyhow!(
                        "field `bashcomp.rhai_script` must include a file path when using relative path notation"
                    ));
                }
                Some(resolve_script_path(&base_dir, rhai_script))
            }
        } else {
            None
        };

        let rhai_function = match bashcomp.rhai_function {
            Some(f) => {
                let function = f.trim();
                if function.is_empty() {
                    None
                } else if function.contains('\0') {
                    return Err(anyhow!(
                        "field `bashcomp.rhai_function` cannot contain NUL bytes"
                    ));
                } else {
                    Some(function.to_string())
                }
            }
            None => None,
        };

        // Cross-field validation: rhai_function without rhai_script is invalid.
        if rhai_function.is_some() && rhai_script.is_none() {
            return Err(anyhow!(
                "field `bashcomp.rhai_function` requires `bashcomp.rhai_script` to be set"
            ));
        }

        manifest.bashcomp = Some(BashcompConfig {
            disabled: bashcomp.disabled,
            passthrough: bashcomp.passthrough,
            script,
            rhai_script,
            rhai_function,
        });
    }

    Ok(manifest)
}

fn normalize_env_map(env: HashMap<String, String>) -> Result<HashMap<String, String>> {
    let mut normalized = HashMap::with_capacity(env.len());
    for (key, value) in env {
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            return Err(anyhow!("field `env` cannot contain empty keys"));
        }
        match env_validation::validate_env_key(normalized_key) {
            Ok(()) => {}
            Err(EnvKeyViolation::ContainsEquals) => {
                return Err(anyhow!(
                    "field `env` keys cannot contain `=`: `{normalized_key}`"
                ));
            }
            Err(EnvKeyViolation::ContainsNul) => {
                return Err(anyhow!("field `env` keys cannot contain NUL bytes"));
            }
        }
        if matches!(
            env_validation::validate_env_value(&value),
            Err(EnvValueViolation::ContainsNul)
        ) {
            return Err(anyhow!(
                "field `env` values cannot contain NUL bytes for key `{normalized_key}`"
            ));
        }
        if normalized.contains_key(normalized_key) {
            return Err(anyhow!(
                "field `env` contains duplicate keys after trimming: `{normalized_key}`"
            ));
        }
        normalized.insert(normalized_key.to_string(), value);
    }
    Ok(normalized)
}

fn normalize_env_remove(env_remove: Vec<String>) -> Result<Vec<String>> {
    let mut seen = HashSet::with_capacity(env_remove.len());
    let mut normalized = Vec::with_capacity(env_remove.len());
    for key in env_remove {
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            continue;
        }
        match env_validation::validate_env_key(normalized_key) {
            Ok(()) => {}
            Err(EnvKeyViolation::ContainsEquals) => {
                return Err(anyhow!(
                    "field `env_remove` entries cannot contain `=`: `{normalized_key}`"
                ));
            }
            Err(EnvKeyViolation::ContainsNul) => {
                return Err(anyhow!(
                    "field `env_remove` entries cannot contain NUL bytes"
                ));
            }
        }
        let normalized_key = normalized_key.to_string();
        if seen.insert(normalized_key.clone()) {
            normalized.push(normalized_key);
        }
    }
    Ok(normalized)
}

fn validate_arg_values(values: &[String], field: &str) -> Result<()> {
    for value in values {
        if matches!(
            arg_validation::validate_arg_value(value),
            Err(ArgViolation::ContainsNul)
        ) {
            return Err(anyhow!("{field} entries cannot contain NUL bytes"));
        }
    }
    Ok(())
}

fn resolve_script_path(base_dir: &Path, script: &str) -> PathBuf {
    let script_path = PathBuf::from(script);
    if script_path.is_absolute() {
        script_path
    } else {
        base_dir.join(script_path)
    }
}

fn config_base_dir(config_path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(config_path) {
        if let Some(parent) = canonical.parent() {
            return parent.to_path_buf();
        }
    }
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn resolve_exec_path(base_dir: &Path, exec: &str) -> PathBuf {
    let exec_path = PathBuf::from(exec);
    if exec_path.is_absolute() {
        return exec_path;
    }

    if looks_like_relative_exec_path(exec) {
        return base_dir.join(exec_path);
    }

    which::which(exec).unwrap_or_else(|_| exec.into())
}

fn looks_like_relative_exec_path(exec: &str) -> bool {
    let path = Path::new(exec);
    !path.is_absolute()
        && (exec == "."
            || exec == ".."
            || exec.contains('/')
            || exec.contains('\\')
            || exec.contains(std::path::MAIN_SEPARATOR))
}

fn has_meaningful_relative_segment(value: &str) -> bool {
    value
        .split(['/', '\\'])
        .any(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
}

fn ends_with_dot_component(value: &str) -> bool {
    let trimmed = value.trim_end_matches(['/', '\\']);
    matches!(trimmed.rsplit(['/', '\\']).next(), Some(".") | Some(".."))
}

#[derive(Debug, Deserialize)]
struct AliasConfig {
    exec: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    env_remove: Vec<String>,
    journal: Option<JournalConfigInput>,
    reconcile: Option<ReconcileConfigInput>,
    bashcomp: Option<BashcompConfigInput>,
}

#[derive(Debug, Deserialize)]
struct JournalConfigInput {
    namespace: String,
    #[serde(default = "default_true")]
    stderr: bool,
    identifier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReconcileConfigInput {
    script: String,
    function: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BashcompConfigInput {
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    passthrough: bool,
    script: Option<String>,
    rhai_script: Option<String>,
    rhai_function: Option<String>,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{looks_like_relative_exec_path, normalize_env_map, normalize_env_remove, parse};
    use anyhow::Result;
    use std::collections::HashMap;
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    #[test]
    fn rejects_non_toml_alias_configs() {
        let temp = TempDir::new().expect("create tempdir");
        let alias = temp.path().join("legacy");
        fs::write(&alias, "echo hello world").expect("write config");

        let err = parse(&alias).expect_err("expected parse failure");
        assert!(err.to_string().contains("expected a .toml file"), "{err}");
    }

    #[test]
    fn parses_toml_alias_with_journal_and_reconcile() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("svc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"
args = ["base"]
env_remove = ["REMOVE_ME"]

[env]
FOO = "bar"

[journal]
namespace = "ops"
stderr = true
identifier = "svc"

[reconcile]
script = "hooks/reconcile.rhai"
"#,
        )
        .expect("write toml");

        let manifest = parse(&config).expect("parse toml config");
        assert_eq!(manifest.args, vec!["base"]);
        assert_eq!(manifest.env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(manifest.env_remove, vec!["REMOVE_ME"]);
        assert_eq!(
            manifest.journal.as_ref().map(|j| j.namespace.as_str()),
            Some("ops")
        );
        assert_eq!(
            manifest
                .reconcile
                .as_ref()
                .map(|r| r.function.as_str())
                .unwrap_or(""),
            "reconcile"
        );
        assert_eq!(
            manifest
                .reconcile
                .as_ref()
                .expect("reconcile config")
                .script,
            temp.path().join("hooks/reconcile.rhai")
        );
    }

    #[test]
    fn parses_toml_alias_with_utf8_bom() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("svc.toml");
        fs::write(
            &config,
            "\u{feff}exec = \"echo\"\nargs = [\"hello\", \"toml\"]\n",
        )
        .expect("write toml");

        let manifest = parse(&config).expect("parse bom toml");
        assert_eq!(
            manifest.exec.file_name().and_then(|x| x.to_str()),
            Some("echo")
        );
        assert_eq!(manifest.args, vec!["hello", "toml"]);
    }

    #[test]
    fn rejects_empty_exec_field_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "   "
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err.to_string().contains("field `exec` cannot be empty"));
    }

    #[test]
    fn rejects_exec_field_containing_nul_bytes() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(&config, "exec = \"echo\\u0000tool\"\n").expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `exec` cannot contain NUL bytes"));
    }

    #[test]
    fn rejects_args_field_containing_nul_bytes() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"
args = ["ok", "bad\u0000arg"]
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `args` entries cannot contain NUL bytes"));
    }

    #[test]
    fn preserves_toml_args_with_empty_unicode_and_whitespace_values() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("ok.toml");
        fs::write(
            &config,
            r#"
exec = "echo"
args = ["", "emoji🚀", " spaced value "]
"#,
        )
        .expect("write toml");

        let manifest = parse(&config).expect("parse args with empty/unicode/whitespace values");
        assert_eq!(
            manifest.args,
            vec![
                "".to_string(),
                "emoji🚀".to_string(),
                " spaced value ".to_string()
            ]
        );
    }

    #[test]
    fn preserves_toml_args_with_symbolic_and_pathlike_values() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("ok.toml");
        fs::write(
            &config,
            r#"
exec = "echo"
args = [
  "--flag=value",
  "../relative/path",
  "semi;colon&and",
  "$DOLLAR",
  "brace{value}",
  'windows\path'
]
"#,
        )
        .expect("write toml");

        let manifest = parse(&config).expect("parse args with symbolic/pathlike values");
        assert_eq!(
            manifest.args,
            vec![
                "--flag=value".to_string(),
                "../relative/path".to_string(),
                "semi;colon&and".to_string(),
                "$DOLLAR".to_string(),
                "brace{value}".to_string(),
                r"windows\path".to_string()
            ]
        );
    }

    #[test]
    fn rejects_dot_exec_field_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `exec` cannot be `.` or `..`"));
    }

    #[test]
    fn rejects_parent_exec_field_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = ".."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `exec` cannot be `.` or `..`"));
    }

    #[test]
    fn rejects_exec_field_ending_in_dot_component() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "bin/.."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `exec` cannot end with `.` or `..` path components"));
    }

    #[test]
    fn rejects_absolute_exec_field_ending_in_dot_component() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "/usr/bin/.."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `exec` cannot end with `.` or `..` path components"));
    }

    #[test]
    fn rejects_dot_slash_exec_field_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "./"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `exec` cannot end with a path separator"));
    }

    #[test]
    fn rejects_trailing_separator_exec_field_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "./bin/"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `exec` cannot end with a path separator"));
    }

    #[test]
    fn rejects_absolute_trailing_separator_exec_field_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "/usr/bin/"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `exec` cannot end with a path separator"));
    }

    #[test]
    fn rejects_absolute_trailing_backslash_exec_field_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = '/usr/bin\'
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `exec` cannot end with a path separator"));
    }

    #[test]
    fn rejects_dot_backslash_exec_field_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = '.\'
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `exec` cannot end with a path separator"));
    }

    #[test]
    fn defaults_reconcile_function_when_blank() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("svc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "hooks/reconcile.rhai"
function = "   "
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(
            manifest
                .reconcile
                .as_ref()
                .map(|r| r.function.as_str())
                .unwrap_or_default(),
            "reconcile"
        );
        Ok(())
    }

    #[test]
    fn defaults_reconcile_function_when_mixed_whitespace_blank() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("svc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "hooks/reconcile.rhai"
function = "\n\t  \t\n"
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(
            manifest
                .reconcile
                .as_ref()
                .map(|r| r.function.as_str())
                .unwrap_or_default(),
            "reconcile"
        );
        Ok(())
    }

    #[test]
    fn rejects_empty_journal_namespace() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[journal]
namespace = "  "
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `journal.namespace` cannot be empty"));
    }

    #[test]
    fn rejects_journal_namespace_containing_nul_bytes() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[journal]
namespace = "ops\u0000prod"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `journal.namespace` cannot contain NUL bytes"));
    }

    #[test]
    fn rejects_journal_identifier_containing_nul_bytes() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[journal]
namespace = "ops"
identifier = "svc\u0000id"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `journal.identifier` cannot contain NUL bytes"));
    }

    #[test]
    fn rejects_dot_reconcile_script_field() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.script` cannot be `.` or `..`"));
    }

    #[test]
    fn rejects_reconcile_script_field_containing_nul_bytes() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "hooks/reconcile\u0000.rhai"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.script` cannot contain NUL bytes"));
    }

    #[test]
    fn rejects_parent_reconcile_script_field() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = ".."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.script` cannot be `.` or `..`"));
    }

    #[test]
    fn rejects_reconcile_script_field_ending_in_dot_component() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "hooks/.."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.script` cannot end with `.` or `..` path components"));
    }

    #[test]
    fn rejects_absolute_reconcile_script_field_ending_in_dot_component() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "/tmp/.."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.script` cannot end with `.` or `..` path components"));
    }

    #[test]
    fn rejects_dot_slash_reconcile_script_field() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "./"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.script` cannot end with a path separator"));
    }

    #[test]
    fn rejects_trailing_separator_reconcile_script_field() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "./hooks/"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.script` cannot end with a path separator"));
    }

    #[test]
    fn rejects_absolute_trailing_separator_reconcile_script_field() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "/tmp/"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.script` cannot end with a path separator"));
    }

    #[test]
    fn rejects_absolute_trailing_backslash_reconcile_script_field() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = '/tmp\'
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.script` cannot end with a path separator"));
    }

    #[test]
    fn rejects_dot_backslash_reconcile_script_field() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = '.\'
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.script` cannot end with a path separator"));
    }

    #[test]
    fn trims_exec_and_journal_fields() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("trimmed.toml");
        fs::write(
            &config,
            r#"
exec = "  echo  "

[journal]
namespace = "  ops  "
identifier = "   "
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(
            manifest.exec.file_name().and_then(|x| x.to_str()),
            Some("echo")
        );
        let journal = manifest.journal.expect("journal config");
        assert_eq!(journal.namespace, "ops");
        assert_eq!(journal.identifier, None);
        Ok(())
    }

    #[test]
    fn trims_mixed_whitespace_exec_and_journal_fields() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("trimmed-mixed.toml");
        fs::write(
            &config,
            r#"
exec = "\n\t echo \t\n"

[journal]
namespace = "\n\t ops \t\n"
identifier = "\n\t  \t\n"
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(
            manifest.exec.file_name().and_then(|x| x.to_str()),
            Some("echo")
        );
        let journal = manifest.journal.expect("journal config");
        assert_eq!(journal.namespace, "ops");
        assert_eq!(journal.identifier, None);
        Ok(())
    }

    #[test]
    fn preserves_journal_symbolic_and_pathlike_namespace_and_identifier() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("journal-symbols.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[journal]
namespace = "  ops/ns.prod@2026  "
identifier = '  svc.id/worker\edge@2026  '
"#,
        )?;

        let manifest = parse(&config)?;
        let journal = manifest.journal.expect("journal config");
        assert_eq!(journal.namespace, "ops/ns.prod@2026");
        assert_eq!(
            journal.identifier.as_deref(),
            Some(r"svc.id/worker\edge@2026")
        );
        Ok(())
    }

    #[test]
    fn trims_reconcile_script_and_function() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("trimmed.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "  hooks/reconcile.rhai  "
function = "  custom_reconcile  "
"#,
        )?;

        let manifest = parse(&config)?;
        let reconcile = manifest.reconcile.expect("reconcile config");
        assert_eq!(reconcile.script, temp.path().join("hooks/reconcile.rhai"));
        assert_eq!(reconcile.function, "custom_reconcile");
        Ok(())
    }

    #[test]
    fn trims_mixed_whitespace_reconcile_script_and_function() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("trimmed-mixed.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "\n\thooks/reconcile.rhai\t\n"
function = "\n\tcustom_reconcile\t\n"
"#,
        )?;

        let manifest = parse(&config)?;
        let reconcile = manifest.reconcile.expect("reconcile config");
        assert_eq!(reconcile.script, temp.path().join("hooks/reconcile.rhai"));
        assert_eq!(reconcile.function, "custom_reconcile");
        Ok(())
    }

    #[test]
    fn rejects_reconcile_function_containing_nul_bytes() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "hooks/reconcile.rhai"
function = "reconcile\u0000hook"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `reconcile.function` cannot contain NUL bytes"));
    }

    #[test]
    fn trims_env_remove_entries_and_drops_blank_values() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("trimmed.toml");
        fs::write(
            &config,
            r#"
exec = "echo"
env_remove = ["  FOO  ", "   ", "BAR"]
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(manifest.env_remove, vec!["FOO", "BAR"]);
        Ok(())
    }

    #[test]
    fn dedupes_env_remove_entries_after_trimming() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("trimmed.toml");
        fs::write(
            &config,
            r#"
exec = "echo"
env_remove = ["  FOO  ", "FOO", "BAR", " BAR "]
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(manifest.env_remove, vec!["FOO", "BAR"]);
        Ok(())
    }

    #[test]
    fn preserves_env_remove_entries_with_symbolic_and_pathlike_shapes() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("env-remove.toml");
        fs::write(
            &config,
            r#"
exec = "echo"
env_remove = [
  " KEY-WITH-DASH ",
  "KEY.WITH.DOT",
  "KEY/WITH/SLASH",
  'KEY\WITH\BACKSLASH',
  "KEY/WITH/SLASH"
]
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(
            manifest.env_remove,
            vec![
                "KEY-WITH-DASH".to_string(),
                "KEY.WITH.DOT".to_string(),
                "KEY/WITH/SLASH".to_string(),
                r"KEY\WITH\BACKSLASH".to_string()
            ]
        );
        Ok(())
    }

    #[test]
    fn preserves_env_values_with_symbolic_and_pathlike_shapes() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("env-values.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[env]
CHOPPER_EQ = "--flag=value"
CHOPPER_REL = "../relative/path"
CHOPPER_SHELL = "semi;colon&and"
CHOPPER_DOLLAR = "$DOLLAR"
" CHOPPER_BRACE " = "brace{value}"
CHOPPER_WIN = 'windows\path'
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(
            manifest.env.get("CHOPPER_EQ").map(String::as_str),
            Some("--flag=value")
        );
        assert_eq!(
            manifest.env.get("CHOPPER_REL").map(String::as_str),
            Some("../relative/path")
        );
        assert_eq!(
            manifest.env.get("CHOPPER_SHELL").map(String::as_str),
            Some("semi;colon&and")
        );
        assert_eq!(
            manifest.env.get("CHOPPER_DOLLAR").map(String::as_str),
            Some("$DOLLAR")
        );
        assert_eq!(
            manifest.env.get("CHOPPER_BRACE").map(String::as_str),
            Some("brace{value}")
        );
        assert_eq!(
            manifest.env.get("CHOPPER_WIN").map(String::as_str),
            Some(r"windows\path")
        );
        Ok(())
    }

    #[test]
    fn preserves_env_keys_with_symbolic_and_pathlike_shapes() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("env-keys.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[env]
" KEY-WITH-DASH " = "dash"
"KEY.WITH.DOT" = "dot"
"KEY/WITH/SLASH" = "slash"
"KEY\\WITH\\BACKSLASH" = "backslash"
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(
            manifest.env.get("KEY-WITH-DASH").map(String::as_str),
            Some("dash")
        );
        assert_eq!(
            manifest.env.get("KEY.WITH.DOT").map(String::as_str),
            Some("dot")
        );
        assert_eq!(
            manifest.env.get("KEY/WITH/SLASH").map(String::as_str),
            Some("slash")
        );
        assert_eq!(
            manifest.env.get(r"KEY\WITH\BACKSLASH").map(String::as_str),
            Some("backslash")
        );
        Ok(())
    }

    #[test]
    fn rejects_env_remove_entries_containing_equals_sign() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"
env_remove = ["BAD=KEY"]
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `env_remove` entries cannot contain `=`"));
    }

    #[test]
    fn rejects_empty_env_keys_after_trimming() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[env]
"   " = "value"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `env` cannot contain empty keys"));
    }

    #[test]
    fn rejects_duplicate_env_keys_after_trimming() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[env]
FOO = "base"
" FOO " = "collision"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("contains duplicate keys after trimming"));
    }

    #[test]
    fn rejects_env_keys_containing_equals_sign() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[env]
"BAD=KEY" = "value"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err.to_string().contains("keys cannot contain `=`"));
    }

    #[test]
    fn rejects_env_keys_containing_nul_bytes() {
        let err = normalize_env_map(HashMap::from([(
            "BAD\0KEY".to_string(),
            "value".to_string(),
        )]))
        .expect_err("expected env key validation failure");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn rejects_env_values_containing_nul_bytes() {
        let err = normalize_env_map(HashMap::from([(
            "GOOD_KEY".to_string(),
            "bad\0value".to_string(),
        )]))
        .expect_err("expected env value validation failure");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn rejects_env_remove_entries_containing_nul_bytes() {
        let err = normalize_env_remove(vec!["BAD\0KEY".to_string()])
            .expect_err("expected env_remove validation failure");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn rejects_env_key_with_nul_escape_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[env]
"BAD\u0000KEY" = "value"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `env` keys cannot contain NUL bytes"));
    }

    #[test]
    fn rejects_env_value_with_nul_escape_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[env]
GOOD_KEY = "bad\u0000value"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `env` values cannot contain NUL bytes"));
    }

    #[test]
    fn rejects_env_remove_entry_with_nul_escape_in_toml() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"
env_remove = ["BAD\u0000KEY"]
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `env_remove` entries cannot contain NUL bytes"));
    }

    #[test]
    fn resolves_reconcile_script_relative_to_symlink_target_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let target_dir = temp.path().join("shared");
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&target_dir)?;
        fs::create_dir_all(&aliases_dir)?;

        let target_config = target_dir.join("linked.toml");
        fs::write(
            &target_config,
            r#"
exec = "echo"

[reconcile]
script = "hooks/reconcile.rhai"
"#,
        )?;
        let symlink_config = aliases_dir.join("linked.toml");
        symlink(&target_config, &symlink_config)?;

        let manifest = parse(&symlink_config)?;
        assert_eq!(
            manifest
                .reconcile
                .as_ref()
                .expect("reconcile config")
                .script,
            target_dir.join("hooks/reconcile.rhai")
        );
        Ok(())
    }

    #[test]
    fn resolves_parent_relative_reconcile_script_against_config_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir)?;
        let config = aliases_dir.join("local.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "../hooks/reconcile.rhai"
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(
            manifest
                .reconcile
                .as_ref()
                .expect("reconcile config")
                .script,
            aliases_dir.join("../hooks/reconcile.rhai")
        );
        Ok(())
    }

    #[test]
    fn resolves_dot_prefixed_reconcile_script_against_config_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir)?;
        let config = aliases_dir.join("local.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = "./hooks/reconcile.rhai"
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(
            manifest
                .reconcile
                .as_ref()
                .expect("reconcile config")
                .script,
            aliases_dir.join("./hooks/reconcile.rhai")
        );
        Ok(())
    }

    #[test]
    fn resolves_symbolic_relative_reconcile_script_against_config_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir)?;
        let config = aliases_dir.join("local.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[reconcile]
script = './hooks/reconcile @v1.rhai'
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(
            manifest
                .reconcile
                .as_ref()
                .expect("reconcile config")
                .script,
            aliases_dir.join("hooks/reconcile @v1.rhai")
        );
        Ok(())
    }

    #[test]
    fn resolves_relative_exec_path_against_config_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir)?;
        let config = aliases_dir.join("local.toml");
        fs::write(
            &config,
            r#"
exec = "bin/runner"
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(manifest.exec, aliases_dir.join("bin/runner"));
        Ok(())
    }

    #[test]
    fn resolves_dot_prefixed_relative_exec_path_against_config_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir)?;
        let config = aliases_dir.join("local.toml");
        fs::write(
            &config,
            r#"
exec = "./bin/runner"
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(manifest.exec, aliases_dir.join("./bin/runner"));
        Ok(())
    }

    #[test]
    fn resolves_symbolic_relative_exec_path_against_config_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir)?;
        let config = aliases_dir.join("local.toml");
        fs::write(
            &config,
            r#"
exec = './bin/runner @v1'
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(manifest.exec, aliases_dir.join("./bin/runner @v1"));
        Ok(())
    }

    #[test]
    fn resolves_parent_relative_exec_path_against_config_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir)?;
        let config = aliases_dir.join("local.toml");
        fs::write(
            &config,
            r#"
exec = "../bin/runner"
"#,
        )?;

        let manifest = parse(&config)?;
        assert_eq!(manifest.exec, aliases_dir.join("../bin/runner"));
        Ok(())
    }

    #[test]
    fn detects_path_like_relative_exec_tokens() {
        assert!(looks_like_relative_exec_path("."));
        assert!(looks_like_relative_exec_path(".."));
        assert!(looks_like_relative_exec_path("./bin/runner"));
        assert!(looks_like_relative_exec_path("../bin/runner"));
        assert!(looks_like_relative_exec_path("bin/runner"));
        assert!(looks_like_relative_exec_path("bin\\runner"));

        assert!(!looks_like_relative_exec_path("echo"));
        assert!(!looks_like_relative_exec_path("kubectl.prod"));
        assert!(!looks_like_relative_exec_path("emoji🚀"));
    }

    #[test]
    fn resolves_relative_exec_path_against_symlink_target_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let target_dir = temp.path().join("shared");
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&target_dir)?;
        fs::create_dir_all(&aliases_dir)?;

        let target_config = target_dir.join("linked.toml");
        fs::write(
            &target_config,
            r#"
exec = "bin/runner"
"#,
        )?;
        let symlink_config = aliases_dir.join("linked.toml");
        symlink(&target_config, &symlink_config)?;

        let manifest = parse(&symlink_config)?;
        assert_eq!(manifest.exec, target_dir.join("bin/runner"));
        Ok(())
    }

    // ------------------------------------------------------------------
    // [bashcomp] table parsing tests
    // ------------------------------------------------------------------

    #[test]
    fn parses_bashcomp_disabled_field() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
disabled = true
"#,
        )?;

        let manifest = parse(&config)?;
        let bashcomp = manifest.bashcomp.expect("bashcomp config");
        assert!(bashcomp.disabled);
        assert!(!bashcomp.passthrough);
        assert!(bashcomp.script.is_none());
        Ok(())
    }

    #[test]
    fn parses_bashcomp_passthrough_field() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
passthrough = true
"#,
        )?;

        let manifest = parse(&config)?;
        let bashcomp = manifest.bashcomp.expect("bashcomp config");
        assert!(!bashcomp.disabled);
        assert!(bashcomp.passthrough);
        assert!(bashcomp.script.is_none());
        Ok(())
    }

    #[test]
    fn parses_bashcomp_script_field() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
script = "completions/custom.bash"
"#,
        )?;

        let manifest = parse(&config)?;
        let bashcomp = manifest.bashcomp.expect("bashcomp config");
        assert!(!bashcomp.disabled);
        assert!(!bashcomp.passthrough);
        assert_eq!(
            bashcomp.script,
            Some(temp.path().join("completions/custom.bash"))
        );
        Ok(())
    }

    #[test]
    fn parses_bashcomp_all_fields() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
disabled = true
passthrough = true
script = "completions/custom.bash"
"#,
        )?;

        let manifest = parse(&config)?;
        let bashcomp = manifest.bashcomp.expect("bashcomp config");
        assert!(bashcomp.disabled);
        assert!(bashcomp.passthrough);
        assert!(bashcomp.script.is_some());
        Ok(())
    }

    #[test]
    fn bashcomp_defaults_to_none_when_absent() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"
"#,
        )?;

        let manifest = parse(&config)?;
        assert!(manifest.bashcomp.is_none());
        Ok(())
    }

    #[test]
    fn bashcomp_empty_table_defaults_fields_to_false() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
"#,
        )?;

        let manifest = parse(&config)?;
        let bashcomp = manifest.bashcomp.expect("bashcomp config");
        assert!(!bashcomp.disabled);
        assert!(!bashcomp.passthrough);
        assert!(bashcomp.script.is_none());
        Ok(())
    }

    #[test]
    fn bashcomp_script_blank_value_is_treated_as_none() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
script = "   "
"#,
        )?;

        let manifest = parse(&config)?;
        let bashcomp = manifest.bashcomp.expect("bashcomp config");
        assert!(bashcomp.script.is_none());
        Ok(())
    }

    #[test]
    fn rejects_bashcomp_script_containing_nul_bytes() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
script = "comp\u0000lete.bash"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `bashcomp.script` cannot contain NUL bytes"));
    }

    #[test]
    fn rejects_bashcomp_script_dot_value() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
script = "."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `bashcomp.script` cannot be `.` or `..`"));
    }

    #[test]
    fn rejects_bashcomp_script_trailing_separator() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
script = "completions/"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `bashcomp.script` cannot end with a path separator"));
    }

    #[test]
    fn rejects_bashcomp_script_ending_in_dot_component() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
script = "completions/.."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `bashcomp.script` cannot end with `.` or `..` path components"));
    }

    // ------------------------------------------------------------------
    // bashcomp.rhai_script / rhai_function tests
    // ------------------------------------------------------------------

    #[test]
    fn parses_bashcomp_rhai_script_field() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
rhai_script = "completions/custom.rhai"
"#,
        )?;

        let manifest = parse(&config)?;
        let bashcomp = manifest.bashcomp.expect("bashcomp config");
        assert_eq!(
            bashcomp.rhai_script,
            Some(temp.path().join("completions/custom.rhai"))
        );
        assert!(bashcomp.rhai_function.is_none());
        Ok(())
    }

    #[test]
    fn parses_bashcomp_rhai_script_and_function() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
rhai_script = "completions/custom.rhai"
rhai_function = "my_completer"
"#,
        )?;

        let manifest = parse(&config)?;
        let bashcomp = manifest.bashcomp.expect("bashcomp config");
        assert_eq!(
            bashcomp.rhai_script,
            Some(temp.path().join("completions/custom.rhai"))
        );
        assert_eq!(bashcomp.rhai_function, Some("my_completer".to_string()));
        Ok(())
    }

    #[test]
    fn bashcomp_rhai_script_blank_value_is_treated_as_none() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
rhai_script = "   "
"#,
        )?;

        let manifest = parse(&config)?;
        let bashcomp = manifest.bashcomp.expect("bashcomp config");
        assert!(bashcomp.rhai_script.is_none());
        Ok(())
    }

    #[test]
    fn bashcomp_rhai_function_blank_value_is_treated_as_none() -> Result<()> {
        let temp = TempDir::new()?;
        let config = temp.path().join("bc.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
rhai_function = "   "
"#,
        )?;

        let manifest = parse(&config)?;
        let bashcomp = manifest.bashcomp.expect("bashcomp config");
        assert!(bashcomp.rhai_function.is_none());
        Ok(())
    }

    #[test]
    fn rejects_bashcomp_rhai_script_containing_nul_bytes() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            "exec = \"echo\"\n\n[bashcomp]\nrhai_script = \"comp\\u0000lete.rhai\"\n",
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `bashcomp.rhai_script` cannot contain NUL bytes"));
    }

    #[test]
    fn rejects_bashcomp_rhai_script_dot_value() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
rhai_script = "."
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `bashcomp.rhai_script` cannot be `.` or `..`"));
    }

    #[test]
    fn rejects_bashcomp_rhai_script_trailing_separator() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
rhai_script = "completions/"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `bashcomp.rhai_script` cannot end with a path separator"));
    }

    #[test]
    fn rejects_bashcomp_rhai_function_containing_nul_bytes() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            "exec = \"echo\"\n\n[bashcomp]\nrhai_script = \"comp.rhai\"\nrhai_function = \"bad\\u0000func\"\n",
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("field `bashcomp.rhai_function` cannot contain NUL bytes"));
    }

    #[test]
    fn rejects_bashcomp_rhai_function_without_rhai_script() {
        let temp = TempDir::new().expect("create tempdir");
        let config = temp.path().join("bad.toml");
        fs::write(
            &config,
            r#"
exec = "echo"

[bashcomp]
rhai_function = "complete"
"#,
        )
        .expect("write toml");

        let err = parse(&config).expect_err("expected parse failure");
        assert!(err
            .to_string()
            .contains("requires `bashcomp.rhai_script` to be set"));
    }
}
