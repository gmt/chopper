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
    pub journal: Option<AliasJournalDoc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AliasJournalDoc {
    pub namespace: String,
    #[serde(default = "default_true")]
    pub stderr: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
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
        Ok(())
    }
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

