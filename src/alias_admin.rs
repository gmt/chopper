use crate::alias_admin_validation::{parse_bool_flag, parse_env_assignment};
use crate::alias_doc::{load_alias_doc, save_alias_doc, AliasDoc, AliasJournalDoc};
use crate::alias_validation;
use anyhow::{anyhow, Context, Result};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoveMode {
    Clean,
    Dirty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MutationInput {
    exec: Option<String>,
    args: Vec<String>,
    env_set: Vec<(String, String)>,
    env_remove: Vec<String>,
    journal_namespace: Option<String>,
    journal_stderr: Option<bool>,
    journal_identifier: Option<String>,
    journal_user_scope: Option<bool>,
    journal_ensure: Option<bool>,
    journal_clear: bool,
}

impl MutationInput {
    fn is_empty(&self) -> bool {
        self.exec.is_none()
            && self.args.is_empty()
            && self.env_set.is_empty()
            && self.env_remove.is_empty()
            && self.journal_namespace.is_none()
            && self.journal_stderr.is_none()
            && self.journal_identifier.is_none()
            && self.journal_user_scope.is_none()
            && self.journal_ensure.is_none()
            && !self.journal_clear
    }
}

pub fn run_alias_action(raw_args: &[String]) -> i32 {
    match run_alias_action_inner(raw_args) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
}

fn run_alias_action_inner(raw_args: &[String]) -> Result<()> {
    if raw_args.is_empty() {
        return Err(anyhow!(
            "usage: chopper --alias <list|get|add|set|remove> ..."
        ));
    }
    match raw_args[0].as_str() {
        "list" => {
            for alias in discover_aliases()? {
                println!("{alias}");
            }
            Ok(())
        }
        "get" => {
            if raw_args.len() != 2 {
                return Err(anyhow!("usage: chopper --alias get <alias>"));
            }
            let alias = &raw_args[1];
            validate_alias(alias)?;
            run_get(alias)
        }
        "add" => run_add_or_set(true, &raw_args[1..]),
        "set" => run_add_or_set(false, &raw_args[1..]),
        "remove" => run_remove(&raw_args[1..]),
        other => Err(anyhow!(
            "unknown alias subcommand `{other}`; expected list|get|add|set|remove"
        )),
    }
}

fn run_get(alias: &str) -> Result<()> {
    let config_path = crate::find_config(alias)
        .ok_or_else(|| anyhow!("alias `{alias}` not found in configuration"))?;
    let manifest = crate::parser::parse(&config_path)?;
    for warning in crate::config_diagnostics::manifest_missing_target_warnings(&manifest) {
        eprintln!("warning: {warning}");
    }
    let output = serde_json::json!({
        "alias": alias,
        "config_path": config_path,
        "exec": manifest.exec,
        "args": manifest.args,
        "env": manifest.env,
        "env_remove": manifest.env_remove,
        "journal": manifest.journal,
        "reconcile": manifest.reconcile,
        "bashcomp": manifest.bashcomp,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&output).context("failed to serialize alias output")?
    );
    Ok(())
}

fn run_add_or_set(is_add: bool, raw_args: &[String]) -> Result<()> {
    if raw_args.is_empty() {
        if is_add {
            return Err(anyhow!(
                "usage: chopper --alias add <alias> --exec <command> ..."
            ));
        }
        return Err(anyhow!("usage: chopper --alias set <alias> [options]"));
    }

    let alias = &raw_args[0];
    validate_alias(alias)?;
    let mutation = parse_mutation_args(&raw_args[1..])?;

    if !is_add && mutation.is_empty() {
        return Err(anyhow!("no changes requested for alias `{alias}`"));
    }

    let target_path = crate::config_dir()
        .join("aliases")
        .join(format!("{alias}.toml"));
    if let Some(parent) = target_path.parent() {
        fs_err::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if is_add {
        if crate::find_config(alias).is_some() {
            return Err(anyhow!(
                "alias `{alias}` already exists; use `set` to modify"
            ));
        }
        let exec = mutation
            .exec
            .clone()
            .ok_or_else(|| anyhow!("`add` requires --exec <command>"))?;
        let journal = build_journal_from_mutation(&mutation, true)?;
        let mut env = HashMap::new();
        for (key, value) in mutation.env_set {
            env.insert(key, value);
        }
        let doc = AliasDoc {
            exec,
            args: mutation.args,
            env,
            env_remove: mutation.env_remove,
            journal,
            reconcile: None,
            bashcomp: None,
        };
        save_alias_doc(&target_path, &doc)?;
        println!("added alias `{alias}` at {}", target_path.display());
        return Ok(());
    }

    let existing_path =
        crate::find_config(alias).ok_or_else(|| anyhow!("alias `{alias}` does not exist"))?;
    let mut doc = load_alias_doc(&existing_path).with_context(|| {
        format!(
            "alias `{alias}` must be a TOML config to use `set` (found {})",
            existing_path.display()
        )
    })?;

    if let Some(exec) = mutation.exec {
        doc.exec = exec;
    }
    if !mutation.args.is_empty() {
        doc.args = mutation.args;
    }
    for (key, value) in mutation.env_set {
        doc.env.insert(key, value);
    }
    if !mutation.env_remove.is_empty() {
        for key in mutation.env_remove {
            if !doc.env_remove.contains(&key) {
                doc.env_remove.push(key);
            }
        }
    }
    if mutation.journal_clear {
        doc.journal = None;
    } else if mutation.journal_namespace.is_some()
        || mutation.journal_stderr.is_some()
        || mutation.journal_identifier.is_some()
        || mutation.journal_user_scope.is_some()
        || mutation.journal_ensure.is_some()
    {
        let mut journal = doc.journal.unwrap_or(AliasJournalDoc {
            namespace: mutation
                .journal_namespace
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            stderr: true,
            identifier: None,
            user_scope: false,
            ensure: false,
        });
        if let Some(namespace) = mutation.journal_namespace {
            journal.namespace = namespace;
        }
        if let Some(stderr) = mutation.journal_stderr {
            journal.stderr = stderr;
        }
        if let Some(identifier) = mutation.journal_identifier {
            if identifier.trim().is_empty() {
                journal.identifier = None;
            } else {
                journal.identifier = Some(identifier);
            }
        }
        if let Some(user_scope) = mutation.journal_user_scope {
            journal.user_scope = user_scope;
        }
        if let Some(ensure) = mutation.journal_ensure {
            journal.ensure = ensure;
        }
        doc.journal = Some(journal);
    }

    save_alias_doc(&existing_path, &doc)?;
    println!("updated alias `{alias}` at {}", existing_path.display());
    Ok(())
}

fn run_remove(raw_args: &[String]) -> Result<()> {
    if raw_args.is_empty() {
        return Err(anyhow!(
            "usage: chopper --alias remove <alias> [--mode clean|dirty] [--symlink-path <path>]"
        ));
    }
    let alias = &raw_args[0];
    validate_alias(alias)?;

    let mut mode = RemoveMode::Clean;
    let mut symlink_path: Option<PathBuf> = None;
    let mut idx = 1;
    while idx < raw_args.len() {
        match raw_args[idx].as_str() {
            "--mode" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--mode requires a value"))?;
                mode = match value.as_str() {
                    "clean" => RemoveMode::Clean,
                    "dirty" => RemoveMode::Dirty,
                    other => {
                        return Err(anyhow!(
                            "unknown remove mode `{other}`; expected clean or dirty"
                        ))
                    }
                };
                idx += 2;
            }
            "--symlink-path" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--symlink-path requires a value"))?;
                symlink_path = Some(PathBuf::from(value));
                idx += 2;
            }
            other => return Err(anyhow!("unknown remove option `{other}`")),
        }
    }

    remove_alias_with_mode(alias, mode, symlink_path)?;
    println!("removed alias `{alias}` ({mode:?})");
    Ok(())
}

pub(crate) fn remove_alias_for_tui(alias: &str, keep_configs: bool) -> Result<()> {
    validate_alias(alias)?;
    let mode = if keep_configs {
        RemoveMode::Dirty
    } else {
        RemoveMode::Clean
    };
    remove_alias_with_mode(alias, mode, None)
}

fn remove_alias_with_mode(
    alias: &str,
    mode: RemoveMode,
    symlink_path: Option<PathBuf>,
) -> Result<()> {
    let mut removed_any = false;
    let symlink_candidate = symlink_path
        .or_else(|| which::which(alias).ok())
        .filter(|path| path.exists());

    match mode {
        RemoveMode::Clean => {
            if let Some(config_path) = crate::find_config(alias) {
                fs_err::remove_file(&config_path).with_context(|| {
                    format!("failed to remove alias config {}", config_path.display())
                })?;
                removed_any = true;
            }
            crate::cache::prune_alias(alias);
            if let Some(path) = symlink_candidate {
                if path.is_symlink() {
                    fs_err::remove_file(&path)
                        .with_context(|| format!("failed to remove symlink {}", path.display()))?;
                    removed_any = true;
                }
            }
        }
        RemoveMode::Dirty => {
            let Some(path) = symlink_candidate else {
                return Err(anyhow!(
                    "keep-configs delete requires a discoverable symlink; pass --symlink-path <path>"
                ));
            };
            if !path.is_symlink() {
                return Err(anyhow!(
                    "keep-configs delete only removes symlinks; `{}` is not a symlink",
                    path.display()
                ));
            }
            fs_err::remove_file(&path)
                .with_context(|| format!("failed to remove symlink {}", path.display()))?;
            removed_any = true;
        }
    }

    if !removed_any {
        return Err(anyhow!("nothing was removed for alias `{alias}`"));
    }
    Ok(())
}

fn parse_mutation_args(raw_args: &[String]) -> Result<MutationInput> {
    let mut exec = None;
    let mut args = Vec::new();
    let mut env_set = Vec::new();
    let mut env_remove = Vec::new();
    let mut journal_namespace = None;
    let mut journal_stderr = None;
    let mut journal_identifier = None;
    let mut journal_user_scope = None;
    let mut journal_ensure = None;
    let mut journal_clear = false;

    let mut idx = 0;
    while idx < raw_args.len() {
        match raw_args[idx].as_str() {
            "--exec" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--exec requires a value"))?;
                exec = Some(value.to_string());
                idx += 2;
            }
            "--arg" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--arg requires a value"))?;
                args.push(value.to_string());
                idx += 2;
            }
            "--env" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--env requires KEY=VALUE"))?;
                env_set.push(parse_env_assignment(value)?);
                idx += 2;
            }
            "--env-remove" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--env-remove requires a key"))?;
                env_remove.push(value.to_string());
                idx += 2;
            }
            "--journal-namespace" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--journal-namespace requires a value"))?;
                journal_namespace = Some(value.to_string());
                idx += 2;
            }
            "--journal-stderr" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--journal-stderr requires true/false"))?;
                journal_stderr = Some(parse_bool_flag(value, "--journal-stderr")?);
                idx += 2;
            }
            "--journal-identifier" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--journal-identifier requires a value"))?;
                journal_identifier = Some(value.to_string());
                idx += 2;
            }
            "--journal-user-scope" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--journal-user-scope requires true/false"))?;
                journal_user_scope = Some(parse_bool_flag(value, "--journal-user-scope")?);
                idx += 2;
            }
            "--journal-ensure" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--journal-ensure requires true/false"))?;
                journal_ensure = Some(parse_bool_flag(value, "--journal-ensure")?);
                idx += 2;
            }
            "--journal-clear" => {
                journal_clear = true;
                idx += 1;
            }
            other => return Err(anyhow!("unknown option `{other}`")),
        }
    }

    Ok(MutationInput {
        exec,
        args,
        env_set,
        env_remove,
        journal_namespace,
        journal_stderr,
        journal_identifier,
        journal_user_scope,
        journal_ensure,
        journal_clear,
    })
}

fn build_journal_from_mutation(
    mutation: &MutationInput,
    allow_none: bool,
) -> Result<Option<AliasJournalDoc>> {
    if mutation.journal_clear {
        return Ok(None);
    }
    if mutation.journal_namespace.is_none()
        && mutation.journal_stderr.is_none()
        && mutation.journal_identifier.is_none()
        && mutation.journal_user_scope.is_none()
        && mutation.journal_ensure.is_none()
    {
        if allow_none {
            return Ok(None);
        }
        return Err(anyhow!(
            "journal mutation requires a namespace or --journal-clear"
        ));
    }
    let namespace = mutation
        .journal_namespace
        .clone()
        .ok_or_else(|| anyhow!("journal namespace is required when setting journal fields"))?;
    let stderr = mutation.journal_stderr.unwrap_or(true);
    let identifier = mutation
        .journal_identifier
        .clone()
        .filter(|value| !value.trim().is_empty());
    let user_scope = mutation.journal_user_scope.unwrap_or(false);
    let ensure = mutation.journal_ensure.unwrap_or(false);
    Ok(Some(AliasJournalDoc {
        namespace,
        stderr,
        identifier,
        user_scope,
        ensure,
    }))
}

pub(crate) fn discover_aliases() -> Result<Vec<String>> {
    let cfg = crate::config_dir();
    let mut aliases = BTreeSet::new();
    discover_aliases_in_dir(&cfg.join("aliases"), &mut aliases)?;
    discover_aliases_in_dir(&cfg, &mut aliases)?;
    Ok(aliases.into_iter().collect())
}

pub(crate) fn default_toml_path(alias: &str) -> PathBuf {
    crate::config_dir()
        .join("aliases")
        .join(format!("{alias}.toml"))
}

pub(crate) fn minimal_alias_doc() -> AliasDoc {
    AliasDoc {
        exec: "echo".to_string(),
        args: Vec::new(),
        env: HashMap::new(),
        env_remove: Vec::new(),
        journal: None,
        reconcile: None,
        bashcomp: None,
    }
}

pub(crate) fn load_or_seed_alias_doc(alias: &str) -> Result<(AliasDoc, PathBuf)> {
    validate_alias(alias)?;
    let Some(config_path) = crate::find_config(alias) else {
        return Ok((minimal_alias_doc(), default_toml_path(alias)));
    };
    let doc = load_alias_doc(&config_path).with_context(|| {
        format!(
            "alias `{alias}` must be a TOML config to edit schema fields (found {})",
            config_path.display()
        )
    })?;
    Ok((doc, config_path))
}

pub(crate) fn save_alias_doc_at(path: &Path, doc: &AliasDoc) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs_err::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    save_alias_doc(path, doc)
}

pub(crate) fn create_alias(alias: &str) -> Result<PathBuf> {
    validate_alias(alias)?;
    if crate::find_config(alias).is_some() {
        return Err(anyhow!(
            "alias `{alias}` already exists; choose a different name"
        ));
    }
    let path = default_toml_path(alias);
    save_alias_doc_at(&path, &minimal_alias_doc())?;
    Ok(path)
}

pub(crate) fn duplicate_alias(source_alias: &str, target_alias: &str) -> Result<PathBuf> {
    validate_alias(source_alias)?;
    validate_alias(target_alias)?;
    if crate::find_config(target_alias).is_some() {
        return Err(anyhow!(
            "alias `{target_alias}` already exists; choose a different name"
        ));
    }
    let source_path = crate::find_config(source_alias)
        .ok_or_else(|| anyhow!("source alias `{source_alias}` does not exist"))?;
    let target_path = sibling_alias_path_for_target(&source_path, target_alias);
    if let Some(parent) = target_path.parent() {
        fs_err::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs_err::copy(&source_path, &target_path).with_context(|| {
        format!(
            "failed to duplicate alias config {} -> {}",
            source_path.display(),
            target_path.display()
        )
    })?;
    Ok(target_path)
}

pub(crate) fn rename_alias(source_alias: &str, target_alias: &str) -> Result<PathBuf> {
    validate_alias(source_alias)?;
    validate_alias(target_alias)?;
    if source_alias == target_alias {
        return Err(anyhow!("rename source and target aliases must differ"));
    }
    if crate::find_config(target_alias).is_some() {
        return Err(anyhow!(
            "alias `{target_alias}` already exists; choose a different name"
        ));
    }
    let source_path = crate::find_config(source_alias)
        .ok_or_else(|| anyhow!("source alias `{source_alias}` does not exist"))?;
    let target_path = sibling_alias_path_for_target(&source_path, target_alias);
    if let Some(parent) = target_path.parent() {
        fs_err::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs_err::rename(&source_path, &target_path).with_context(|| {
        format!(
            "failed to rename alias config {} -> {}",
            source_path.display(),
            target_path.display()
        )
    })?;
    crate::cache::prune_alias(source_alias);
    crate::cache::prune_alias(target_alias);
    Ok(target_path)
}

fn sibling_alias_path_for_target(source_path: &Path, target_alias: &str) -> PathBuf {
    let parent = source_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(crate::config_dir);
    parent.join(format!("{target_alias}.toml"))
}

fn discover_aliases_in_dir(dir: &Path, aliases: &mut BTreeSet<String>) -> Result<()> {
    let entries = match fs_err::read_dir(dir) {
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

fn validate_alias(alias: &str) -> Result<()> {
    alias_validation::validate_alias_identifier(alias)
        .map_err(|_| anyhow!("invalid alias identifier `{alias}`"))
}

#[cfg(test)]
mod tests {
    use super::{
        create_alias, duplicate_alias, load_or_seed_alias_doc, parse_mutation_args, rename_alias,
    };
    use crate::test_support::ENV_LOCK;
    use std::env;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_add_mutation_flags() {
        let mutation = parse_mutation_args(&[
            "--exec".into(),
            "echo".into(),
            "--arg".into(),
            "hello".into(),
            "--env".into(),
            "A=1".into(),
            "--env-remove".into(),
            "OLD".into(),
            "--journal-namespace".into(),
            "ops".into(),
            "--journal-stderr".into(),
            "false".into(),
            "--journal-identifier".into(),
            "svc".into(),
            "--journal-user-scope".into(),
            "true".into(),
            "--journal-ensure".into(),
            "true".into(),
        ])
        .expect("mutation parse");
        assert_eq!(mutation.exec.as_deref(), Some("echo"));
        assert_eq!(mutation.args, vec!["hello"]);
        assert_eq!(mutation.env_set, vec![("A".into(), "1".into())]);
        assert_eq!(mutation.env_remove, vec!["OLD"]);
        assert_eq!(mutation.journal_namespace.as_deref(), Some("ops"));
        assert_eq!(mutation.journal_stderr, Some(false));
        assert_eq!(mutation.journal_identifier.as_deref(), Some("svc"));
        assert_eq!(mutation.journal_user_scope, Some(true));
        assert_eq!(mutation.journal_ensure, Some(true));
    }

    #[test]
    fn create_alias_writes_default_toml_file() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let temp = TempDir::new().expect("tempdir");
        env::set_var("CHOPPER_CONFIG_DIR", temp.path());
        let path = create_alias("newalias").expect("create alias");
        assert!(path.is_file(), "expected created file {}", path.display());
        let content = fs::read_to_string(&path).expect("read created alias file");
        assert!(content.contains("exec"), "{content}");
        env::remove_var("CHOPPER_CONFIG_DIR");
    }

    #[test]
    fn duplicate_and_rename_alias_preserve_file_kind() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let temp = TempDir::new().expect("tempdir");
        let cfg = temp.path();
        fs::create_dir_all(cfg).expect("create cfg");
        fs::write(cfg.join("source.toml"), "exec = \"echo\"\n").expect("write source");
        env::set_var("CHOPPER_CONFIG_DIR", cfg);

        let duplicate_path = duplicate_alias("source", "copy").expect("duplicate alias");
        assert_eq!(
            duplicate_path.file_name().and_then(|v| v.to_str()),
            Some("copy.toml")
        );
        assert!(duplicate_path.is_file());

        let renamed_path = rename_alias("copy", "renamed").expect("rename alias");
        assert_eq!(
            renamed_path.file_name().and_then(|v| v.to_str()),
            Some("renamed.toml")
        );
        assert!(renamed_path.is_file());
        assert!(!duplicate_path.exists());

        env::remove_var("CHOPPER_CONFIG_DIR");
    }

    #[test]
    fn load_or_seed_alias_doc_builds_default_for_missing_alias() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let temp = TempDir::new().expect("tempdir");
        env::set_var("CHOPPER_CONFIG_DIR", temp.path());
        let (doc, path) = load_or_seed_alias_doc("missing").expect("seed alias doc");
        assert_eq!(doc.exec, "echo");
        assert!(path.ends_with("missing.toml"));
        env::remove_var("CHOPPER_CONFIG_DIR");
    }
}
