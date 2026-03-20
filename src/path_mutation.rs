use crate::path_mutation_validation::{self, PathMutationViolation};
use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathMutationConfig {
    #[serde(default)]
    pub remove_all: Vec<String>,
    #[serde(default)]
    pub remove_one: Vec<String>,
    #[serde(default)]
    pub append_all: Vec<String>,
    #[serde(default)]
    pub append_one: Vec<String>,
    #[serde(default)]
    pub prepend_all: Vec<String>,
    #[serde(default)]
    pub prepend_one: Vec<String>,
}

impl PathMutationConfig {
    pub fn is_empty(&self) -> bool {
        self.remove_all.is_empty()
            && self.remove_one.is_empty()
            && self.append_all.is_empty()
            && self.append_one.is_empty()
            && self.prepend_all.is_empty()
            && self.prepend_one.is_empty()
    }

    pub fn validate(&self, field_prefix: &str) -> Result<()> {
        validate_values(&self.remove_all, field_prefix, "remove_all")?;
        validate_values(&self.remove_one, field_prefix, "remove_one")?;
        validate_values(&self.append_all, field_prefix, "append_all")?;
        validate_values(&self.append_one, field_prefix, "append_one")?;
        validate_values(&self.prepend_all, field_prefix, "prepend_all")?;
        validate_values(&self.prepend_one, field_prefix, "prepend_one")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinglePathOpKind {
    RemoveAll,
    RemoveOne,
    AppendAll,
    AppendOne,
    PrependAll,
    PrependOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PathIdentity {
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    #[cfg(not(unix))]
    canonical: PathBuf,
}

#[derive(Default)]
struct MatcherState {
    canonical_identities: HashMap<String, Option<PathIdentity>>,
    regexes: HashMap<String, Regex>,
}

pub fn apply_runtime_path(base_path: Option<&str>, config: &PathMutationConfig) -> Result<String> {
    let components = split_runtime_path(base_path);
    let components = apply_components(components, config, "path")?;
    join_runtime_path(&components)
}

pub fn apply_single_colon_list_op(
    list: &str,
    kind: SinglePathOpKind,
    operand: &str,
    context: &str,
) -> Result<String> {
    match path_mutation_validation::validate_path_mutation_value(list) {
        Ok(()) => {}
        Err(PathMutationViolation::ContainsNul) => {
            return Err(anyhow!("{context} list cannot contain NUL bytes"));
        }
    }
    match path_mutation_validation::validate_path_mutation_value(operand) {
        Ok(()) => {}
        Err(PathMutationViolation::ContainsNul) => {
            return Err(anyhow!("{context} operand cannot contain NUL bytes"));
        }
    }

    let mut components = split_colon_list(list);
    let mut state = MatcherState::default();
    match kind {
        SinglePathOpKind::RemoveAll => {
            remove_all_matching(&mut components, operand, context, &mut state)?
        }
        SinglePathOpKind::RemoveOne => {
            remove_first_matching(&mut components, operand, context, &mut state)?
        }
        SinglePathOpKind::AppendAll => {
            remove_all_equivalent(&mut components, operand, &mut state);
            components.push(operand.to_string());
        }
        SinglePathOpKind::AppendOne => {
            remove_first_equivalent(&mut components, operand, &mut state);
            components.push(operand.to_string());
        }
        SinglePathOpKind::PrependAll => {
            remove_all_equivalent(&mut components, operand, &mut state);
            components.insert(0, operand.to_string());
        }
        SinglePathOpKind::PrependOne => {
            remove_first_equivalent(&mut components, operand, &mut state);
            components.insert(0, operand.to_string());
        }
    }
    Ok(join_colon_list(&components))
}

pub fn split_colon_list(list: &str) -> Vec<String> {
    list.split(':').map(ToString::to_string).collect()
}

pub fn join_colon_list(components: &[String]) -> String {
    components.join(":")
}

fn apply_components(
    mut components: Vec<String>,
    config: &PathMutationConfig,
    field_prefix: &str,
) -> Result<Vec<String>> {
    let mut state = MatcherState::default();

    for pattern in &config.remove_all {
        remove_all_matching(
            &mut components,
            pattern,
            &format!("{field_prefix}.remove_all"),
            &mut state,
        )?;
    }
    for pattern in &config.remove_one {
        remove_first_matching(
            &mut components,
            pattern,
            &format!("{field_prefix}.remove_one"),
            &mut state,
        )?;
    }
    for path in &config.append_all {
        remove_all_equivalent(&mut components, path, &mut state);
        components.push(path.clone());
    }
    for path in &config.append_one {
        remove_first_equivalent(&mut components, path, &mut state);
        components.push(path.clone());
    }
    for path in &config.prepend_all {
        remove_all_equivalent(&mut components, path, &mut state);
        components.insert(0, path.clone());
    }
    for path in &config.prepend_one {
        remove_first_equivalent(&mut components, path, &mut state);
        components.insert(0, path.clone());
    }

    Ok(components)
}

fn validate_values(values: &[String], field_prefix: &str, field_name: &str) -> Result<()> {
    for value in values {
        match path_mutation_validation::validate_path_mutation_value(value) {
            Ok(()) => {}
            Err(PathMutationViolation::ContainsNul) => {
                return Err(anyhow!(
                    "field `{field_prefix}.{field_name}` entries cannot contain NUL bytes"
                ));
            }
        }
    }
    Ok(())
}

fn split_runtime_path(base_path: Option<&str>) -> Vec<String> {
    let Some(base_path) = base_path else {
        return Vec::new();
    };

    env::split_paths(base_path)
        .map(|value| value.to_string_lossy().to_string())
        .collect()
}

fn join_runtime_path(components: &[String]) -> Result<String> {
    let joined = env::join_paths(components.iter().map(PathBuf::from))
        .map_err(|err| anyhow!("failed to join PATH components: {err}"))?;
    joined
        .into_string()
        .map_err(|_| anyhow!("effective PATH cannot be represented as UTF-8"))
}

fn remove_first_matching(
    components: &mut Vec<String>,
    pattern: &str,
    context: &str,
    state: &mut MatcherState,
) -> Result<()> {
    let regex = compile_regex(pattern, context, state)?;
    if let Some(index) = components
        .iter()
        .position(|component| regex.is_match(component))
    {
        components.remove(index);
    }
    Ok(())
}

fn remove_all_matching(
    components: &mut Vec<String>,
    pattern: &str,
    context: &str,
    state: &mut MatcherState,
) -> Result<()> {
    let regex = compile_regex(pattern, context, state)?;
    components.retain(|component| !regex.is_match(component));
    Ok(())
}

fn compile_regex<'a>(
    pattern: &'a str,
    context: &str,
    state: &'a mut MatcherState,
) -> Result<&'a Regex> {
    if !state.regexes.contains_key(pattern) {
        let regex = Regex::new(pattern)
            .map_err(|err| anyhow!("{context} contains invalid regex `{pattern}`: {err}"))?;
        state.regexes.insert(pattern.to_string(), regex);
    }
    Ok(state
        .regexes
        .get(pattern)
        .expect("regex cache should contain compiled pattern"))
}

fn remove_first_equivalent(components: &mut Vec<String>, path: &str, state: &mut MatcherState) {
    if let Some(index) = components
        .iter()
        .position(|component| components_equivalent(component, path, state))
    {
        components.remove(index);
    }
}

fn remove_all_equivalent(components: &mut Vec<String>, path: &str, state: &mut MatcherState) {
    components.retain(|component| !components_equivalent(component, path, state));
}

fn components_equivalent(left: &str, right: &str, state: &mut MatcherState) -> bool {
    let Some(left_identity) = canonical_identity(left, state) else {
        return false;
    };
    let Some(right_identity) = canonical_identity(right, state) else {
        return false;
    };
    left_identity == right_identity
}

fn canonical_identity(value: &str, state: &mut MatcherState) -> Option<PathIdentity> {
    if let Some(identity) = state.canonical_identities.get(value) {
        return *identity;
    }

    let canonical = std::fs::canonicalize(value).ok();
    let identity = canonical.and_then(|path| path_identity(&path));
    state
        .canonical_identities
        .insert(value.to_string(), identity);
    identity
}

#[cfg(unix)]
fn path_identity(path: &std::path::Path) -> Option<PathIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::metadata(path).ok()?;
    Some(PathIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
    })
}

#[cfg(not(unix))]
fn path_identity(path: &std::path::Path) -> Option<PathIdentity> {
    Some(PathIdentity {
        canonical: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        apply_runtime_path, apply_single_colon_list_op, join_colon_list, split_colon_list,
        PathMutationConfig, SinglePathOpKind,
    };
    use anyhow::Result;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn split_and_join_colon_lists_round_trip_empty_components() {
        let components = split_colon_list(":/usr/bin::/bin:");
        assert_eq!(components, vec!["", "/usr/bin", "", "/bin", ""]);
        assert_eq!(join_colon_list(&components), ":/usr/bin::/bin:");
    }

    #[test]
    fn remove_operations_run_before_additions() -> Result<()> {
        let temp = TempDir::new()?;
        let remove_target = temp.path().join("remove-target");
        let keep_target = temp.path().join("keep-target");
        let prepend_target = temp.path().join("prepend-target");
        let append_target = temp.path().join("append-target");
        fs::create_dir(&remove_target)?;
        fs::create_dir(&keep_target)?;
        fs::create_dir(&prepend_target)?;
        fs::create_dir(&append_target)?;

        let config = PathMutationConfig {
            remove_all: vec![format!(
                "^{}$",
                regex::escape(&remove_target.display().to_string())
            )],
            append_one: vec![append_target.display().to_string()],
            prepend_one: vec![prepend_target.display().to_string()],
            ..PathMutationConfig::default()
        };
        let out = apply_single_colon_list_op(
            &format!(
                "{}:{}:{}",
                remove_target.display(),
                keep_target.display(),
                append_target.display()
            ),
            SinglePathOpKind::RemoveAll,
            &format!("^{}$", regex::escape(&remove_target.display().to_string())),
            "pathlist_remove_all",
        )?;
        assert_eq!(
            out,
            format!("{}:{}", keep_target.display(), append_target.display())
        );

        let out = super::apply_components(
            vec![
                remove_target.display().to_string(),
                keep_target.display().to_string(),
                append_target.display().to_string(),
            ],
            &config,
            "path",
        )?;
        assert_eq!(
            out,
            vec![
                prepend_target.display().to_string(),
                keep_target.display().to_string(),
                append_target.display().to_string(),
            ]
        );
        Ok(())
    }

    #[test]
    fn append_one_removes_only_first_equivalent_entry() -> Result<()> {
        let temp = TempDir::new()?;
        let real = temp.path().join("real");
        fs::create_dir(&real)?;
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&real, &alias)?;

        let config = PathMutationConfig {
            append_one: vec![real.display().to_string()],
            ..PathMutationConfig::default()
        };
        let out = super::apply_components(
            vec![
                alias.display().to_string(),
                alias.display().to_string(),
                "/bin".to_string(),
            ],
            &config,
            "path",
        )?;
        assert_eq!(
            out,
            vec![
                alias.display().to_string(),
                "/bin".to_string(),
                real.display().to_string()
            ]
        );
        Ok(())
    }

    #[test]
    fn append_all_removes_all_equivalent_entries_before_inserting() -> Result<()> {
        let temp = TempDir::new()?;
        let real = temp.path().join("real");
        fs::create_dir(&real)?;
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&real, &alias)?;

        let config = PathMutationConfig {
            append_all: vec![real.display().to_string()],
            ..PathMutationConfig::default()
        };
        let out = super::apply_components(
            vec![
                alias.display().to_string(),
                "/bin".to_string(),
                alias.display().to_string(),
            ],
            &config,
            "path",
        )?;
        assert_eq!(out, vec!["/bin".to_string(), real.display().to_string()]);
        Ok(())
    }

    #[test]
    fn non_canonicalizable_paths_do_not_match_for_append_dedup() -> Result<()> {
        let config = PathMutationConfig {
            append_all: vec!["/definitely/missing".to_string()],
            ..PathMutationConfig::default()
        };
        let out =
            super::apply_components(vec!["/definitely/missing".to_string()], &config, "path")?;
        assert_eq!(
            out,
            vec![
                "/definitely/missing".to_string(),
                "/definitely/missing".to_string()
            ]
        );
        Ok(())
    }

    #[test]
    fn remove_one_uses_regex_against_raw_components() -> Result<()> {
        let out = apply_single_colon_list_op(
            "/tmp/a:/tmp/b:/usr/bin",
            SinglePathOpKind::RemoveOne,
            r"^/tmp/",
            "pathlist_remove_one",
        )?;
        assert_eq!(out, "/tmp/b:/usr/bin");
        Ok(())
    }

    #[test]
    fn invalid_regex_reports_context() {
        let err = apply_single_colon_list_op(
            "/bin:/usr/bin",
            SinglePathOpKind::RemoveAll,
            "(",
            "pathlist_remove_all",
        )
        .expect_err("invalid regex should fail");
        assert!(
            err.to_string()
                .contains("pathlist_remove_all contains invalid regex `(`"),
            "{err}"
        );
    }

    #[test]
    fn runtime_path_uses_empty_base_when_missing() -> Result<()> {
        let config = PathMutationConfig {
            prepend_one: vec!["/custom/bin".to_string()],
            ..PathMutationConfig::default()
        };
        let out = apply_runtime_path(None, &config)?;
        assert_eq!(out, "/custom/bin");
        Ok(())
    }
}
