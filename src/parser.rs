use crate::manifest::{JournalConfig, Manifest, ReconcileConfig};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn parse(path: &Path) -> Result<Manifest> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read alias config {}", path.display()))?;

    if is_toml_path(path) {
        parse_toml(&content, path)
    } else {
        parse_trivial(&content)
    }
}

fn is_toml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("toml"))
        .unwrap_or(false)
}

fn parse_trivial(content: &str) -> Result<Manifest> {
    let line = content
        .lines()
        .next()
        .ok_or_else(|| anyhow!("empty config file"))?
        .trim();

    if line.is_empty() {
        return Err(anyhow!("empty config file"));
    }

    let parts = shell_words::split(line)?;
    if parts.is_empty() {
        return Err(anyhow!("no command found"));
    }

    let exec = which::which(&parts[0]).unwrap_or_else(|_| parts[0].clone().into());

    let args = parts[1..].to_vec();

    Ok(Manifest::simple(exec).with_args(args))
}

fn parse_toml(content: &str, path: &Path) -> Result<Manifest> {
    let parsed: AliasConfig =
        toml::from_str(content).with_context(|| format!("invalid TOML in {}", path.display()))?;

    let exec = parsed.exec.trim();
    if exec.is_empty() {
        return Err(anyhow!("field `exec` cannot be empty"));
    }

    let exec = which::which(exec).unwrap_or_else(|_| exec.into());

    let mut manifest = Manifest::simple(exec).with_args(parsed.args);
    manifest.env = normalize_env_map(parsed.env)?;
    manifest.env_remove = normalize_env_remove(parsed.env_remove);

    if let Some(journal) = parsed.journal {
        let namespace = journal.namespace.trim();
        if namespace.is_empty() {
            return Err(anyhow!("field `journal.namespace` cannot be empty"));
        }
        let identifier = journal
            .identifier
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        manifest = manifest.with_journal(JournalConfig {
            namespace: namespace.to_string(),
            stderr: journal.stderr,
            identifier,
        });
    }

    if let Some(reconcile) = parsed.reconcile {
        let script = reconcile.script.trim();
        if script.is_empty() {
            return Err(anyhow!("field `reconcile.script` cannot be empty"));
        }
        let script = resolve_script_path(path, script);
        let function = reconcile
            .function
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| "reconcile".to_string());
        manifest = manifest.with_reconcile(ReconcileConfig { script, function });
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
        normalized.insert(normalized_key.to_string(), value);
    }
    Ok(normalized)
}

fn normalize_env_remove(env_remove: Vec<String>) -> Vec<String> {
    env_remove
        .into_iter()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .collect()
}

fn resolve_script_path(config_path: &Path, script: &str) -> PathBuf {
    let script_path = PathBuf::from(script);
    if script_path.is_absolute() {
        script_path
    } else {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(script_path)
    }
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

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::parse;
    use anyhow::Result;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_trivial_legacy_alias() {
        let temp = TempDir::new().expect("create tempdir");
        let alias = temp.path().join("legacy");
        fs::write(&alias, "echo hello world").expect("write config");

        let manifest = parse(&alias).expect("parse legacy config");
        assert_eq!(
            manifest.exec.file_name().and_then(|x| x.to_str()),
            Some("echo")
        );
        assert_eq!(manifest.args, vec!["hello", "world"]);
        assert!(manifest.journal.is_none());
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
}
