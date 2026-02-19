use crate::arg_validation::{self, ArgViolation};
use crate::env_validation::{self, EnvKeyViolation, EnvValueViolation};
use crate::journal_validation::{self, JournalIdentifierViolation, JournalNamespaceViolation};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AliasDoc {
    pub exec: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub env_remove: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal: Option<AliasJournalDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconcile: Option<AliasReconcileDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bashcomp: Option<AliasBashcompDoc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AliasJournalDoc {
    pub namespace: String,
    #[serde(default = "default_true")]
    pub stderr: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(default)]
    pub user_scope: bool,
    #[serde(default)]
    pub ensure: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_use: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_interval_usec: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_burst: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AliasReconcileDoc {
    pub script: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AliasBashcompDoc {
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub passthrough: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rhai_script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rhai_function: Option<String>,
}

fn default_true() -> bool {
    true
}

impl AliasDoc {
    pub fn validate(&self) -> Result<()> {
        if self.exec.trim().is_empty() {
            return Err(anyhow!("`exec` cannot be blank"));
        }
        if self.exec.contains('\0') {
            return Err(anyhow!("`exec` cannot contain NUL bytes"));
        }
        for arg in &self.args {
            if matches!(
                arg_validation::validate_arg_value(arg),
                Err(ArgViolation::ContainsNul)
            ) {
                return Err(anyhow!("`args` entries cannot contain NUL bytes"));
            }
        }
        for (key, value) in &self.env {
            match env_validation::validate_env_key(key) {
                Ok(()) => {}
                Err(EnvKeyViolation::ContainsEquals) => {
                    return Err(anyhow!("`env` key `{key}` cannot contain `=`"));
                }
                Err(EnvKeyViolation::ContainsNul) => {
                    return Err(anyhow!("`env` key `{key}` cannot contain NUL bytes"));
                }
            }
            if matches!(
                env_validation::validate_env_value(value),
                Err(EnvValueViolation::ContainsNul)
            ) {
                return Err(anyhow!(
                    "`env` value for key `{key}` cannot contain NUL bytes"
                ));
            }
        }
        for key in &self.env_remove {
            match env_validation::validate_env_key(key) {
                Ok(()) => {}
                Err(EnvKeyViolation::ContainsEquals) => {
                    return Err(anyhow!("`env_remove` key `{key}` cannot contain `=`"));
                }
                Err(EnvKeyViolation::ContainsNul) => {
                    return Err(anyhow!("`env_remove` key `{key}` cannot contain NUL bytes"));
                }
            }
        }
        if let Some(journal) = &self.journal {
            match journal_validation::normalize_namespace(&journal.namespace) {
                Ok(_) => {}
                Err(JournalNamespaceViolation::Empty) => {
                    return Err(anyhow!("`journal.namespace` cannot be blank"));
                }
                Err(JournalNamespaceViolation::ContainsNul) => {
                    return Err(anyhow!("`journal.namespace` cannot contain NUL bytes"));
                }
            }
            match journal_validation::normalize_optional_identifier_for_invocation(
                journal.identifier.as_deref(),
            ) {
                Ok(_) => {}
                Err(JournalIdentifierViolation::Blank) => {
                    return Err(anyhow!(
                        "`journal.identifier` cannot be blank when provided"
                    ));
                }
                Err(JournalIdentifierViolation::ContainsNul) => {
                    return Err(anyhow!("`journal.identifier` cannot contain NUL bytes"));
                }
            }
        }
        if let Some(reconcile) = &self.reconcile {
            validate_required_script_field(&reconcile.script, "`reconcile.script`")?;
            if let Some(function) = &reconcile.function {
                if function.contains('\0') {
                    return Err(anyhow!("`reconcile.function` cannot contain NUL bytes"));
                }
            }
        }
        if let Some(bashcomp) = &self.bashcomp {
            validate_optional_script_field(bashcomp.script.as_deref(), "`bashcomp.script`")?;
            validate_optional_script_field(
                bashcomp.rhai_script.as_deref(),
                "`bashcomp.rhai_script`",
            )?;
            if let Some(function) = &bashcomp.rhai_function {
                if function.contains('\0') {
                    return Err(anyhow!("`bashcomp.rhai_function` cannot contain NUL bytes"));
                }
                if !function.trim().is_empty()
                    && !bashcomp
                        .rhai_script
                        .as_deref()
                        .map(|value| !value.trim().is_empty())
                        .unwrap_or(false)
                {
                    return Err(anyhow!(
                        "`bashcomp.rhai_function` requires `bashcomp.rhai_script`"
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_required_script_field(value: &str, field: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("{field} cannot be blank"));
    }
    validate_script_shape(trimmed, field)
}

fn validate_optional_script_field(value: Option<&str>, field: &str) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    validate_script_shape(trimmed, field)
}

fn validate_script_shape(trimmed: &str, field: &str) -> Result<()> {
    if trimmed.contains('\0') {
        return Err(anyhow!("{field} cannot contain NUL bytes"));
    }
    if trimmed == "." || trimmed == ".." {
        return Err(anyhow!("{field} cannot be `.` or `..`"));
    }
    if trimmed.ends_with('/') || trimmed.ends_with('\\') {
        return Err(anyhow!("{field} cannot end with a path separator"));
    }
    if ends_with_dot_component(trimmed) {
        return Err(anyhow!(
            "{field} cannot end with `.` or `..` path components"
        ));
    }
    if !Path::new(trimmed).is_absolute() && !has_meaningful_relative_segment(trimmed) {
        return Err(anyhow!(
            "{field} must include a file path when using relative path notation"
        ));
    }
    Ok(())
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

pub fn load_alias_doc(path: &Path) -> Result<AliasDoc> {
    let content = fs_err::read_to_string(path)
        .with_context(|| format!("failed to read alias config {}", path.display()))?;
    let parsed: AliasDoc = toml::from_str(&content)
        .with_context(|| format!("failed to parse TOML {}", path.display()))?;
    parsed.validate()?;
    Ok(parsed)
}

pub fn save_alias_doc(path: &Path, doc: &AliasDoc) -> Result<()> {
    doc.validate()?;
    let content =
        toml::to_string_pretty(doc).context("failed to serialize alias config to TOML")?;
    fs_err::write(path, content)
        .with_context(|| format!("failed to write alias config {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        load_alias_doc, save_alias_doc, AliasBashcompDoc, AliasDoc, AliasJournalDoc,
        AliasReconcileDoc,
    };
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn valid_doc() -> AliasDoc {
        AliasDoc {
            exec: "echo".to_string(),
            args: vec!["hello".to_string()],
            env: HashMap::from([("A".to_string(), "1".to_string())]),
            env_remove: vec!["OLD".to_string()],
            journal: Some(AliasJournalDoc {
                namespace: "ops".to_string(),
                stderr: true,
                identifier: Some("svc".to_string()),
                user_scope: false,
                ensure: false,
                max_use: None,
                rate_limit_interval_usec: None,
                rate_limit_burst: None,
            }),
            reconcile: Some(AliasReconcileDoc {
                script: "hooks/reconcile.rhai".to_string(),
                function: Some("reconcile".to_string()),
            }),
            bashcomp: Some(AliasBashcompDoc {
                disabled: false,
                passthrough: true,
                script: Some("comp/custom.bash".to_string()),
                rhai_script: Some("comp/custom.rhai".to_string()),
                rhai_function: Some("complete".to_string()),
            }),
        }
    }

    #[test]
    fn alias_doc_round_trips_through_toml_persistence() {
        let temp = TempDir::new().expect("tempdir");
        let path = temp.path().join("alias.toml");
        let doc = valid_doc();
        save_alias_doc(&path, &doc).expect("save alias doc");
        let loaded = load_alias_doc(&path).expect("load alias doc");
        assert_eq!(loaded, doc);
    }

    #[test]
    fn alias_doc_validation_rejects_blank_exec() {
        let mut doc = valid_doc();
        doc.exec = "   ".to_string();
        let err = doc.validate().expect_err("blank exec should fail");
        assert!(err.to_string().contains("`exec` cannot be blank"));
    }

    #[test]
    fn alias_doc_validation_rejects_env_key_with_equals() {
        let mut doc = valid_doc();
        doc.env.insert("BAD=KEY".to_string(), "value".to_string());
        let err = doc.validate().expect_err("equals in env key should fail");
        assert!(err.to_string().contains("cannot contain `=`"));
    }

    #[test]
    fn alias_doc_validation_rejects_blank_journal_identifier() {
        let mut doc = valid_doc();
        doc.journal = Some(AliasJournalDoc {
            namespace: "ops".to_string(),
            stderr: true,
            identifier: Some("   ".to_string()),
            user_scope: false,
            ensure: false,
            max_use: None,
            rate_limit_interval_usec: None,
            rate_limit_burst: None,
        });
        let err = doc
            .validate()
            .expect_err("blank journal identifier should fail");
        assert!(err.to_string().contains("cannot be blank"));
    }

    #[test]
    fn alias_doc_validation_rejects_blank_reconcile_script() {
        let mut doc = valid_doc();
        doc.reconcile = Some(AliasReconcileDoc {
            script: "   ".to_string(),
            function: Some("reconcile".to_string()),
        });
        let err = doc
            .validate()
            .expect_err("blank reconcile script should fail");
        assert!(err
            .to_string()
            .contains("`reconcile.script` cannot be blank"));
    }

    #[test]
    fn alias_doc_validation_requires_rhai_script_when_function_set() {
        let mut doc = valid_doc();
        doc.bashcomp = Some(AliasBashcompDoc {
            disabled: false,
            passthrough: false,
            script: None,
            rhai_script: None,
            rhai_function: Some("complete".to_string()),
        });
        let err = doc
            .validate()
            .expect_err("rhai function without script should fail");
        assert!(err.to_string().contains("requires `bashcomp.rhai_script`"));
    }
}
