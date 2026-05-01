use crate::alias_admin_parse::{parse_bool_flag, parse_env_assignment};
use crate::alias_doc::{load_alias_doc, save_alias_doc, AliasDoc, AliasJournalDoc};
use crate::alias_validation;
use crate::path_mutation::PathMutationConfig;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AliasMutationMode {
    Add,
    Set,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoveMode {
    Clean,
    Dirty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathField {
    RemoveAll,
    RemoveOne,
    AppendAll,
    AppendOne,
    PrependAll,
    PrependOne,
}

impl PathField {
    fn toml_name(self) -> &'static str {
        match self {
            PathField::RemoveAll => "path.remove_all",
            PathField::RemoveOne => "path.remove_one",
            PathField::AppendAll => "path.append_all",
            PathField::AppendOne => "path.append_one",
            PathField::PrependAll => "path.prepend_all",
            PathField::PrependOne => "path.prepend_one",
        }
    }

    fn flag_name(self) -> &'static str {
        match self {
            PathField::RemoveAll => "--path-remove-all",
            PathField::RemoveOne => "--path-remove-one",
            PathField::AppendAll => "--path-append-all",
            PathField::AppendOne => "--path-append-one",
            PathField::PrependAll => "--path-prepend-all",
            PathField::PrependOne => "--path-prepend-one",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathFieldReplacement {
    field: PathField,
    values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MutationInput {
    exec: Option<String>,
    args: Vec<String>,
    env_set: Vec<(String, String)>,
    env_remove: Vec<String>,
    path_remove_all: Vec<String>,
    path_remove_one: Vec<String>,
    path_append_all: Vec<String>,
    path_append_one: Vec<String>,
    path_prepend_all: Vec<String>,
    path_prepend_one: Vec<String>,
    path_replacement: Option<PathFieldReplacement>,
    journal_namespace: Option<String>,
    journal_stderr: Option<bool>,
    journal_identifier: Option<String>,
    journal_user_scope: Option<bool>,
    journal_ensure: Option<bool>,
    journal_max_use: Option<String>,
    journal_rate_limit_interval_usec: Option<u64>,
    journal_rate_limit_burst: Option<u32>,
    journal_clear: bool,
    no_wrapper_sync: bool,
}

impl MutationInput {
    fn is_empty(&self) -> bool {
        self.exec.is_none()
            && self.args.is_empty()
            && self.env_set.is_empty()
            && self.env_remove.is_empty()
            && self.path_remove_all.is_empty()
            && self.path_remove_one.is_empty()
            && self.path_append_all.is_empty()
            && self.path_append_one.is_empty()
            && self.path_prepend_all.is_empty()
            && self.path_prepend_one.is_empty()
            && self.path_replacement.is_none()
            && self.journal_namespace.is_none()
            && self.journal_stderr.is_none()
            && self.journal_identifier.is_none()
            && self.journal_user_scope.is_none()
            && self.journal_ensure.is_none()
            && self.journal_max_use.is_none()
            && self.journal_rate_limit_interval_usec.is_none()
            && self.journal_rate_limit_burst.is_none()
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
        print_alias_help();
        return Ok(());
    }
    match raw_args[0].as_str() {
        "-h" | "--help" | "help" => {
            // chopper --alias --help  or  chopper --alias help
            // Subcommand-specific help: chopper --alias help get
            if let Some(sub) = raw_args.get(1) {
                print_alias_subcommand_help(sub)?;
            } else {
                print_alias_help();
            }
            Ok(())
        }
        "get" => {
            if raw_args.get(1).map(String::as_str) == Some("--help") {
                return print_alias_subcommand_help("get");
            }
            if raw_args.len() != 2 {
                return Err(anyhow!("usage: chopper --alias get <alias>"));
            }
            let alias = &raw_args[1];
            validate_alias(alias)?;
            run_get(alias)
        }
        "add" => {
            if raw_args.get(1).map(String::as_str) == Some("--help") {
                return print_alias_subcommand_help("add");
            }
            run_add_or_set(AliasMutationMode::Add, &raw_args[1..])
        }
        "set" => {
            if raw_args.get(1).map(String::as_str) == Some("--help") {
                return print_alias_subcommand_help("set");
            }
            run_add_or_set(AliasMutationMode::Set, &raw_args[1..])
        }
        "remove" => {
            if raw_args.get(1).map(String::as_str) == Some("--help") {
                return print_alias_subcommand_help("remove");
            }
            run_remove(&raw_args[1..])
        }
        other => Err(anyhow!(
            "unknown alias subcommand `{other}`; expected get|add|set|remove\n\
             Run `chopper --alias --help` for usage."
        )),
    }
}

fn print_alias_help() {
    println!("Manage chopper alias configurations.");
    println!();
    println!("Usage:");
    println!("  chopper --alias <subcommand> [options]");
    println!();
    println!("Subcommands:");
    println!("  get <alias>");
    println!("      Print the configuration for an alias as JSON.");
    println!();
    println!("  add <alias> --exec <command> [options]");
    println!("      Create a new alias. Fails if the alias already exists.");
    println!("      Requires --exec.");
    println!("      By default, creates/updates a wrapper symlink in ~/bin or ~/.local/bin.");
    println!();
    println!("  set <alias> [options]");
    println!("      Update fields on an existing alias. At least one option required.");
    println!();
    println!("  remove <alias> [--mode clean|dirty] [--symlink-path <path>] [--no-wrapper-sync]");
    println!("      Remove an alias config and its symlink.");
    println!("      --mode clean (default): remove config file and symlink");
    println!("      --mode dirty: remove symlink only, keep config file");
    println!();
    println!("Options for add/set:");
    println!("  --exec <command>           Command to execute");
    println!("  --arg <value>              Append a fixed argument (repeatable)");
    println!("  --env KEY=VALUE            Set an environment variable (repeatable)");
    println!("  --env-remove KEY           Unset an environment variable (repeatable)");
    println!("  --path-remove-all REGEX    Remove all matching PATH entries (repeatable)");
    println!("  --path-remove-one REGEX    Remove first matching PATH entry (repeatable)");
    println!("  --path-append-all PATH     Append PATH after removing all equivalent entries (repeatable)");
    println!("  --path-append-one PATH     Append PATH after removing first equivalent entry (repeatable)");
    println!("  --path-prepend-all PATH    Prepend PATH after removing all equivalent entries (repeatable)");
    println!("  --path-prepend-one PATH    Prepend PATH after removing first equivalent entry (repeatable)");
    println!("                             With `set`, a path option replaces that field from the rest of argv.");
    println!("                             Place it last; use `--` before values if helpful.");
    println!("  --journal-namespace <ns>   systemd journal namespace (enables journaling)");
    println!("  --journal-stderr true|false  Capture stderr to journal (default: true)");
    println!("  --journal-identifier <id>  Journal syslog identifier override");
    println!("  --journal-user-scope true|false  Use user-scope journal (default: false)");
    println!("  --journal-ensure true|false  Fail if journald is unavailable (default: false)");
    println!("  --journal-max-use <size>   Maximum journal disk space (e.g. 500M)");
    println!("  --journal-rate-limit-interval-usec <usec>  Rate-limit window (microseconds)");
    println!("  --journal-rate-limit-burst <n>             Max messages per window");
    println!("  --journal-clear            Remove all journal settings from alias");
    println!("  --no-wrapper-sync          Skip automatic wrapper create/remove");
    println!();
    println!("Examples:");
    println!("  chopper --alias add mygrep --exec grep --arg -n");
    println!("  chopper --alias set mygrep --env GREP_COLORS=auto");
    println!("  chopper --alias get mygrep");
    println!("  chopper --alias remove mygrep");
    println!();
    println!("Run `chopper --alias <subcommand> --help` for subcommand-specific details.");
}

fn print_alias_subcommand_help(sub: &str) -> Result<()> {
    match sub {
        "get" => {
            println!("Usage: chopper --alias get <alias>");
            println!();
            println!("Print the full configuration for an alias as formatted JSON.");
            println!(
                "Includes exec, args, env, env_remove, path, journal, reconcile, and bashcomp fields."
            );
            println!("When present, path includes remove_all, remove_one, append_all, append_one,");
            println!("prepend_all, and prepend_one.");
            println!();
            println!("Example:");
            println!("  chopper --alias get mygrep");
        }
        "add" => {
            println!("Usage: chopper --alias add <alias> --exec <command> [options]");
            println!();
            println!("Create a new alias TOML config. Fails if the alias already exists.");
            println!("Use `set` to modify an existing alias.");
            println!("By default, writes/refreshes a wrapper symlink in ~/bin or ~/.local/bin.");
            println!();
            println!("Required:");
            println!("  --exec <command>           Command to execute");
            println!();
            println!("Options:");
            println!("  --arg <value>              Append a fixed argument (repeatable)");
            println!("  --env KEY=VALUE            Set an environment variable (repeatable)");
            println!("  --env-remove KEY           Unset an environment variable at runtime (repeatable)");
            println!("  --path-remove-all REGEX    Remove all matching PATH entries (repeatable)");
            println!("  --path-remove-one REGEX    Remove first matching PATH entry (repeatable)");
            println!("  --path-append-all PATH     Append PATH after removing all equivalent entries (repeatable)");
            println!("  --path-append-one PATH     Append PATH after removing first equivalent entry (repeatable)");
            println!("  --path-prepend-all PATH    Prepend PATH after removing all equivalent entries (repeatable)");
            println!("  --path-prepend-one PATH    Prepend PATH after removing first equivalent entry (repeatable)");
            println!("  --journal-namespace <ns>   Enable systemd journaling under this namespace");
            println!("  --journal-stderr true|false  Capture stderr to journal (default: true)");
            println!(
                "  --journal-identifier <id>  Override the syslog identifier in journal entries"
            );
            println!(
                "  --journal-user-scope true|false  Use user-scope journal unit (default: false)"
            );
            println!("  --journal-ensure true|false  Hard-fail if journald is unavailable (default: false)");
            println!(
                "  --journal-max-use <size>   Max journal disk usage for this alias (e.g. 500M)"
            );
            println!("  --journal-rate-limit-interval-usec <usec>  Rate-limit window length");
            println!("  --journal-rate-limit-burst <n>             Max log messages per window");
            println!("  --no-wrapper-sync          Skip automatic wrapper symlink creation");
            println!();
            println!("Examples:");
            println!("  chopper --alias add mygrep --exec grep --arg -n");
            println!("  chopper --alias add syslog-svc --exec /usr/bin/myapp \\");
            println!("      --journal-namespace default --journal-stderr true");
        }
        "set" => {
            println!("Usage: chopper --alias set <alias> [options]");
            println!();
            println!(
                "Update one or more fields on an existing alias. At least one option is required."
            );
            println!("Use `add` to create a new alias.");
            println!();
            println!("Options:");
            println!("  --exec <command>           Replace the exec command");
            println!("  --arg <value>              Replace fixed args with this set (repeatable)");
            println!(
                "  --env KEY=VALUE            Add or update an environment variable (repeatable)"
            );
            println!("  --env-remove KEY           Add a key to the runtime env-remove list (repeatable)");
            println!("  --path-remove-all [--] ... Replace path.remove_all with regexes from the rest of argv");
            println!("  --path-remove-one [--] ... Replace path.remove_one with regexes from the rest of argv");
            println!("  --path-append-all [--] ... Replace path.append_all with paths from the rest of argv");
            println!("  --path-append-one [--] ... Replace path.append_one with paths from the rest of argv");
            println!("  --path-prepend-all [--] ... Replace path.prepend_all with paths from the rest of argv");
            println!("  --path-prepend-one [--] ... Replace path.prepend_one with paths from the rest of argv");
            println!("                             Path replacement options consume the rest of argv and may be empty to clear.");
            println!("  --journal-namespace <ns>   Set/update the journal namespace");
            println!("  --journal-stderr true|false  Update stderr capture setting");
            println!("  --journal-identifier <id>  Update the syslog identifier (empty string clears it)");
            println!("  --journal-user-scope true|false  Update user-scope journal setting");
            println!("  --journal-ensure true|false  Update journald availability enforcement");
            println!("  --journal-max-use <size>   Update max journal disk usage (empty string clears it)");
            println!("  --journal-rate-limit-interval-usec <usec>  Update rate-limit window");
            println!("  --journal-rate-limit-burst <n>             Update rate-limit burst count");
            println!("  --journal-clear            Remove all journal settings from the alias");
            println!();
            println!("Examples:");
            println!("  chopper --alias set mygrep --exec rg");
            println!("  chopper --alias set mygrep --env GREP_COLORS=always --env-remove OLD_VAR");
            println!(
                "  chopper --alias set mygrep --path-append-one -- /usr/local/bin /opt/tools/bin"
            );
            println!("  chopper --alias set mygrep --journal-clear");
        }
        "remove" => {
            println!("Usage: chopper --alias remove <alias> [--mode clean|dirty] [--symlink-path <path>] [--no-wrapper-sync]");
            println!();
            println!("Remove an alias and/or its symlink.");
            println!();
            println!("Options:");
            println!("  --mode clean (default)  Remove both the config file and the symlink");
            println!("  --mode dirty            Remove only the symlink; keep the config file");
            println!(
                "  --symlink-path <path>   Explicit symlink path to remove (overrides PATH lookup)"
            );
            println!("  --no-wrapper-sync       Skip automatic wrapper symlink removal");
            println!();
            println!("Examples:");
            println!("  chopper --alias remove mygrep");
            println!("  chopper --alias remove mygrep --mode dirty");
            println!("  chopper --alias remove mygrep --symlink-path ~/.local/bin/mygrep");
        }
        other => {
            return Err(anyhow!(
                "unknown alias subcommand `{other}`; expected get|add|set|remove\n\
                 Run `chopper --alias --help` for usage."
            ));
        }
    }
    Ok(())
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
        "path": manifest.path,
        "journal": manifest.journal,
        "reconcile": manifest.reconcile,
        "bashcomp": manifest.bashcomp,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&output).context("failed to serialize alias output")?
    );
    emit_wrapper_warnings(alias);
    Ok(())
}

fn run_add_or_set(mode: AliasMutationMode, raw_args: &[String]) -> Result<()> {
    if raw_args.is_empty() {
        if mode == AliasMutationMode::Add {
            return Err(anyhow!(
                "usage: chopper --alias add <alias> --exec <command> ..."
            ));
        }
        return Err(anyhow!("usage: chopper --alias set <alias> [options]"));
    }

    let alias = &raw_args[0];
    validate_alias(alias)?;
    let mutation = parse_mutation_args(&raw_args[1..], mode)?;

    if mode == AliasMutationMode::Set && mutation.is_empty() {
        return Err(anyhow!("no changes requested for alias `{alias}`"));
    }

    let target_path = default_toml_path(alias);
    if let Some(parent) = target_path.parent() {
        fs_err::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if mode == AliasMutationMode::Add {
        if crate::find_config(alias).is_some() {
            return Err(anyhow!(
                "alias `{alias}` already exists; use `set` to modify"
            ));
        }
        let exec = mutation
            .exec
            .clone()
            .ok_or_else(|| anyhow!("`add` requires --exec <command>"))?;
        let path = path_doc_from_mutation(&mutation);
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
            path,
            journal,
            reconcile: None,
            bashcomp: None,
        };
        save_alias_doc(&target_path, &doc)?;
        if !mutation.no_wrapper_sync {
            emit_warnings(crate::wrapper_sync::ensure_wrapper(alias)?);
        }
        emit_wrapper_warnings(alias);
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
    let path_mutation = path_doc_from_mutation(&mutation);
    let mut replaced_path_field = None;

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
    if let Some(path_mutation) = path_mutation {
        let path = doc.path.get_or_insert_with(PathMutationConfig::default);
        path.remove_all.extend(path_mutation.remove_all);
        path.remove_one.extend(path_mutation.remove_one);
        path.append_all.extend(path_mutation.append_all);
        path.append_one.extend(path_mutation.append_one);
        path.prepend_all.extend(path_mutation.prepend_all);
        path.prepend_one.extend(path_mutation.prepend_one);
        if path.is_empty() {
            doc.path = None;
        }
    }
    if let Some(replacement) = mutation.path_replacement {
        let path = doc.path.get_or_insert_with(PathMutationConfig::default);
        let previous_values = path_field_values(path, replacement.field).to_vec();
        replace_path_field(path, replacement.field, replacement.values.clone());
        if path.is_empty() {
            doc.path = None;
        }
        replaced_path_field = Some((replacement, previous_values));
    }
    if mutation.journal_clear {
        doc.journal = None;
    } else if mutation.journal_namespace.is_some()
        || mutation.journal_stderr.is_some()
        || mutation.journal_identifier.is_some()
        || mutation.journal_user_scope.is_some()
        || mutation.journal_ensure.is_some()
        || mutation.journal_max_use.is_some()
        || mutation.journal_rate_limit_interval_usec.is_some()
        || mutation.journal_rate_limit_burst.is_some()
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
            max_use: None,
            rate_limit_interval_usec: None,
            rate_limit_burst: None,
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
        if let Some(max_use) = mutation.journal_max_use {
            if max_use.trim().is_empty() {
                journal.max_use = None;
            } else {
                journal.max_use = Some(max_use);
            }
        }
        if let Some(interval) = mutation.journal_rate_limit_interval_usec {
            journal.rate_limit_interval_usec = Some(interval);
        }
        if let Some(burst) = mutation.journal_rate_limit_burst {
            journal.rate_limit_burst = Some(burst);
        }
        doc.journal = Some(journal);
    }

    save_alias_doc(&existing_path, &doc)?;
    emit_wrapper_warnings(alias);
    if let Some((replacement, previous_values)) = replaced_path_field {
        emit_path_replacement_summary(alias, &replacement, &previous_values);
    }
    println!("updated alias `{alias}` at {}", existing_path.display());
    Ok(())
}

fn run_remove(raw_args: &[String]) -> Result<()> {
    if raw_args.is_empty() {
        return Err(anyhow!(
            "usage: chopper --alias remove <alias> [--mode clean|dirty] [--symlink-path <path>] [--no-wrapper-sync]"
        ));
    }
    let alias = &raw_args[0];
    validate_alias(alias)?;

    let mut mode = RemoveMode::Clean;
    let mut symlink_path: Option<PathBuf> = None;
    let mut no_wrapper_sync = false;
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
            "--no-wrapper-sync" => {
                no_wrapper_sync = true;
                idx += 1;
            }
            other => return Err(anyhow!("unknown remove option `{other}`")),
        }
    }
    if mode == RemoveMode::Dirty && no_wrapper_sync {
        return Err(anyhow!(
            "--mode dirty cannot be combined with --no-wrapper-sync"
        ));
    }

    remove_alias_with_mode(alias, mode, symlink_path, no_wrapper_sync)?;
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
    remove_alias_with_mode(alias, mode, None, false)
}

fn remove_alias_with_mode(
    alias: &str,
    mode: RemoveMode,
    symlink_path: Option<PathBuf>,
    no_wrapper_sync: bool,
) -> Result<()> {
    let mut removed_any = false;

    match mode {
        RemoveMode::Clean => {
            if let Some(config_path) = crate::find_config(alias) {
                fs_err::remove_file(&config_path).with_context(|| {
                    format!("failed to remove alias config {}", config_path.display())
                })?;
                crate::alias_paths::try_remove_empty_alias_dir(
                    &crate::config_dir(),
                    alias,
                    &config_path,
                );
                removed_any = true;
            }
            crate::cache::prune_alias(alias);
            if !no_wrapper_sync && crate::wrapper_sync::remove_wrapper(alias, symlink_path)? {
                removed_any = true;
            }
        }
        RemoveMode::Dirty => {
            if crate::wrapper_sync::remove_wrapper(alias, symlink_path)? {
                removed_any = true;
            }
        }
    }

    if !removed_any {
        if mode == RemoveMode::Dirty {
            return Err(anyhow!(
                "keep-configs delete requires an existing wrapper symlink; pass --symlink-path <path> if needed"
            ));
        }
        return Err(anyhow!("nothing was removed for alias `{alias}`"));
    }
    Ok(())
}

fn emit_warnings(warnings: Vec<String>) {
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
}

fn emit_wrapper_warnings(alias: &str) {
    emit_warnings(crate::wrapper_sync::wrapper_health_warnings(alias));
}

fn parse_mutation_args(raw_args: &[String], mode: AliasMutationMode) -> Result<MutationInput> {
    let mut exec = None;
    let mut args = Vec::new();
    let mut env_set = Vec::new();
    let mut env_remove = Vec::new();
    let mut path_remove_all = Vec::new();
    let mut path_remove_one = Vec::new();
    let mut path_append_all = Vec::new();
    let mut path_append_one = Vec::new();
    let mut path_prepend_all = Vec::new();
    let mut path_prepend_one = Vec::new();
    let mut path_replacement = None;
    let mut journal_namespace = None;
    let mut journal_stderr = None;
    let mut journal_identifier = None;
    let mut journal_user_scope = None;
    let mut journal_ensure = None;
    let mut journal_max_use = None;
    let mut journal_rate_limit_interval_usec = None;
    let mut journal_rate_limit_burst = None;
    let mut journal_clear = false;
    let mut no_wrapper_sync = false;

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
            "--path-remove-all" => {
                if mode == AliasMutationMode::Set {
                    path_replacement = Some(parse_path_replacement(
                        PathField::RemoveAll,
                        &raw_args[idx + 1..],
                    ));
                    break;
                }
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--path-remove-all requires a regex"))?;
                path_remove_all.push(value.to_string());
                idx += 2;
            }
            "--path-remove-one" => {
                if mode == AliasMutationMode::Set {
                    path_replacement = Some(parse_path_replacement(
                        PathField::RemoveOne,
                        &raw_args[idx + 1..],
                    ));
                    break;
                }
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--path-remove-one requires a regex"))?;
                path_remove_one.push(value.to_string());
                idx += 2;
            }
            "--path-append-all" => {
                if mode == AliasMutationMode::Set {
                    path_replacement = Some(parse_path_replacement(
                        PathField::AppendAll,
                        &raw_args[idx + 1..],
                    ));
                    break;
                }
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--path-append-all requires a path"))?;
                path_append_all.push(value.to_string());
                idx += 2;
            }
            "--path-append-one" => {
                if mode == AliasMutationMode::Set {
                    path_replacement = Some(parse_path_replacement(
                        PathField::AppendOne,
                        &raw_args[idx + 1..],
                    ));
                    break;
                }
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--path-append-one requires a path"))?;
                path_append_one.push(value.to_string());
                idx += 2;
            }
            "--path-prepend-all" => {
                if mode == AliasMutationMode::Set {
                    path_replacement = Some(parse_path_replacement(
                        PathField::PrependAll,
                        &raw_args[idx + 1..],
                    ));
                    break;
                }
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--path-prepend-all requires a path"))?;
                path_prepend_all.push(value.to_string());
                idx += 2;
            }
            "--path-prepend-one" => {
                if mode == AliasMutationMode::Set {
                    path_replacement = Some(parse_path_replacement(
                        PathField::PrependOne,
                        &raw_args[idx + 1..],
                    ));
                    break;
                }
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--path-prepend-one requires a path"))?;
                path_prepend_one.push(value.to_string());
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
            "--journal-max-use" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--journal-max-use requires a value"))?;
                journal_max_use = Some(value.to_string());
                idx += 2;
            }
            "--journal-rate-limit-interval-usec" => {
                let value = raw_args.get(idx + 1).ok_or_else(|| {
                    anyhow!("--journal-rate-limit-interval-usec requires a value")
                })?;
                journal_rate_limit_interval_usec = Some(value.parse::<u64>().map_err(|_| {
                    anyhow!("--journal-rate-limit-interval-usec requires a positive integer")
                })?);
                idx += 2;
            }
            "--journal-rate-limit-burst" => {
                let value = raw_args
                    .get(idx + 1)
                    .ok_or_else(|| anyhow!("--journal-rate-limit-burst requires a value"))?;
                journal_rate_limit_burst = Some(value.parse::<u32>().map_err(|_| {
                    anyhow!("--journal-rate-limit-burst requires a positive integer")
                })?);
                idx += 2;
            }
            "--journal-clear" => {
                journal_clear = true;
                idx += 1;
            }
            "--no-wrapper-sync" => {
                no_wrapper_sync = true;
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
        path_remove_all,
        path_remove_one,
        path_append_all,
        path_append_one,
        path_prepend_all,
        path_prepend_one,
        path_replacement,
        journal_namespace,
        journal_stderr,
        journal_identifier,
        journal_user_scope,
        journal_ensure,
        journal_max_use,
        journal_rate_limit_interval_usec,
        journal_rate_limit_burst,
        journal_clear,
        no_wrapper_sync,
    })
}

fn mutation_has_path_changes(mutation: &MutationInput) -> bool {
    !mutation.path_remove_all.is_empty()
        || !mutation.path_remove_one.is_empty()
        || !mutation.path_append_all.is_empty()
        || !mutation.path_append_one.is_empty()
        || !mutation.path_prepend_all.is_empty()
        || !mutation.path_prepend_one.is_empty()
}

fn parse_path_replacement(field: PathField, raw_values: &[String]) -> PathFieldReplacement {
    let values = match raw_values.first().map(String::as_str) {
        Some("--") => raw_values[1..].to_vec(),
        _ => raw_values.to_vec(),
    };
    PathFieldReplacement { field, values }
}

fn path_doc_from_mutation(mutation: &MutationInput) -> Option<PathMutationConfig> {
    if !mutation_has_path_changes(mutation) {
        return None;
    }
    Some(PathMutationConfig {
        remove_all: mutation.path_remove_all.clone(),
        remove_one: mutation.path_remove_one.clone(),
        append_all: mutation.path_append_all.clone(),
        append_one: mutation.path_append_one.clone(),
        prepend_all: mutation.path_prepend_all.clone(),
        prepend_one: mutation.path_prepend_one.clone(),
    })
}

fn path_field_values(path: &PathMutationConfig, field: PathField) -> &[String] {
    match field {
        PathField::RemoveAll => &path.remove_all,
        PathField::RemoveOne => &path.remove_one,
        PathField::AppendAll => &path.append_all,
        PathField::AppendOne => &path.append_one,
        PathField::PrependAll => &path.prepend_all,
        PathField::PrependOne => &path.prepend_one,
    }
}

fn replace_path_field(path: &mut PathMutationConfig, field: PathField, values: Vec<String>) {
    match field {
        PathField::RemoveAll => path.remove_all = values,
        PathField::RemoveOne => path.remove_one = values,
        PathField::AppendAll => path.append_all = values,
        PathField::AppendOne => path.append_one = values,
        PathField::PrependAll => path.prepend_all = values,
        PathField::PrependOne => path.prepend_one = values,
    }
}

fn emit_path_replacement_summary(
    alias: &str,
    replacement: &PathFieldReplacement,
    previous: &[String],
) {
    eprintln!(
        "new value of {}: {}",
        replacement.field.toml_name(),
        serde_json::to_string(&replacement.values).unwrap_or_else(|_| "[]".to_string())
    );
    let mut restore_args = vec![
        "chopper".to_string(),
        "--alias".to_string(),
        "set".to_string(),
        alias.to_string(),
        replacement.field.flag_name().to_string(),
        "--".to_string(),
    ];
    restore_args.extend(previous.iter().cloned());
    eprintln!("restore previous value with:");
    eprintln!("{}", shell_words::join(restore_args));
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
        && mutation.journal_max_use.is_none()
        && mutation.journal_rate_limit_interval_usec.is_none()
        && mutation.journal_rate_limit_burst.is_none()
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
    let user_scope = mutation.journal_user_scope.unwrap_or(true);
    let ensure = mutation.journal_ensure.unwrap_or(false);
    let max_use = mutation.journal_max_use.clone();
    let rate_limit_interval_usec = mutation.journal_rate_limit_interval_usec;
    let rate_limit_burst = mutation.journal_rate_limit_burst;
    Ok(Some(AliasJournalDoc {
        namespace,
        stderr,
        identifier,
        user_scope,
        ensure,
        max_use,
        rate_limit_interval_usec,
        rate_limit_burst,
    }))
}

pub(crate) fn discover_aliases() -> Result<Vec<String>> {
    crate::alias_paths::discover_exec_aliases(&crate::config_dir())
}

pub(crate) fn default_toml_path(alias: &str) -> PathBuf {
    crate::alias_paths::default_exec_config_path(&crate::config_dir(), alias)
}

pub(crate) fn minimal_alias_doc() -> AliasDoc {
    AliasDoc {
        exec: "echo".to_string(),
        args: Vec::new(),
        env: HashMap::new(),
        env_remove: Vec::new(),
        path: None,
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
    let target_path = sibling_alias_path_for_target(&source_path, source_alias, target_alias);
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
    let target_path = sibling_alias_path_for_target(&source_path, source_alias, target_alias);
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

fn sibling_alias_path_for_target(
    source_path: &Path,
    source_alias: &str,
    target_alias: &str,
) -> PathBuf {
    crate::alias_paths::target_path_like_source(
        &crate::config_dir(),
        source_path,
        source_alias,
        target_alias,
    )
}

fn validate_alias(alias: &str) -> Result<()> {
    use crate::alias_validation::AliasViolation;
    alias_validation::validate_alias_identifier(alias).map_err(|v| match v {
        AliasViolation::Empty => anyhow!("alias name cannot be empty"),
        AliasViolation::ContainsNul => anyhow!("alias name cannot contain NUL bytes"),
        AliasViolation::IsSeparator => {
            anyhow!("alias name cannot be `--`; expected `chopper <alias> -- [args...]`")
        }
        AliasViolation::StartsWithDash => {
            anyhow!("alias name cannot start with `-`; choose a non-flag alias name")
        }
        AliasViolation::ContainsWhitespace => {
            anyhow!("alias name cannot contain whitespace")
        }
        AliasViolation::IsDotToken => anyhow!("alias name cannot be `.` or `..`"),
        AliasViolation::ContainsPathSeparator => anyhow!(
            "alias name cannot contain path separators; \
             use symlink mode or command PATH resolution instead"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        create_alias, duplicate_alias, load_or_seed_alias_doc, parse_mutation_args, rename_alias,
        AliasMutationMode, PathField,
    };
    use crate::test_support::ENV_LOCK;
    use std::env;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_add_mutation_flags() {
        let mutation = parse_mutation_args(
            &[
                "--exec".into(),
                "echo".into(),
                "--arg".into(),
                "hello".into(),
                "--env".into(),
                "A=1".into(),
                "--env-remove".into(),
                "OLD".into(),
                "--path-remove-all".into(),
                "^/tmp".into(),
                "--path-append-one".into(),
                "/custom/bin".into(),
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
                "--no-wrapper-sync".into(),
            ],
            AliasMutationMode::Add,
        )
        .expect("mutation parse");
        assert_eq!(mutation.exec.as_deref(), Some("echo"));
        assert_eq!(mutation.args, vec!["hello"]);
        assert_eq!(mutation.env_set, vec![("A".into(), "1".into())]);
        assert_eq!(mutation.env_remove, vec!["OLD"]);
        assert_eq!(mutation.path_remove_all, vec!["^/tmp"]);
        assert_eq!(mutation.path_append_one, vec!["/custom/bin"]);
        assert_eq!(mutation.journal_namespace.as_deref(), Some("ops"));
        assert_eq!(mutation.journal_stderr, Some(false));
        assert_eq!(mutation.journal_identifier.as_deref(), Some("svc"));
        assert_eq!(mutation.journal_user_scope, Some(true));
        assert_eq!(mutation.journal_ensure, Some(true));
        assert!(mutation.no_wrapper_sync);
    }

    #[test]
    fn parses_set_path_replacement_with_optional_separator() {
        let mutation = parse_mutation_args(
            &[
                "--env".into(),
                "A=1".into(),
                "--path-append-one".into(),
                "--".into(),
                "/usr/local/bin".into(),
                "/opt/tools/bin".into(),
            ],
            AliasMutationMode::Set,
        )
        .expect("mutation parse");

        assert_eq!(mutation.env_set, vec![("A".into(), "1".into())]);
        assert_eq!(
            mutation.path_replacement,
            Some(super::PathFieldReplacement {
                field: PathField::AppendOne,
                values: vec!["/usr/local/bin".into(), "/opt/tools/bin".into()],
            })
        );
    }

    #[test]
    fn parses_set_path_replacement_with_empty_tail_to_clear() {
        let mutation = parse_mutation_args(&["--path-remove-all".into()], AliasMutationMode::Set)
            .expect("mutation parse");

        assert_eq!(
            mutation.path_replacement,
            Some(super::PathFieldReplacement {
                field: PathField::RemoveAll,
                values: Vec::new(),
            })
        );
    }

    #[test]
    fn create_alias_writes_default_toml_file() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let temp = TempDir::new().expect("tempdir");
        env::set_var("CHOPPER_CONFIG_DIR", temp.path());
        let path = create_alias("newalias").expect("create alias");
        assert_eq!(path, temp.path().join("newalias/exe.toml"));
        assert!(path.is_file(), "expected created file {}", path.display());
        let content = fs::read_to_string(&path).expect("read created alias file");
        assert!(content.contains("exec"), "{content}");
        env::remove_var("CHOPPER_CONFIG_DIR");
    }

    #[test]
    fn canonical_aliases_use_alias_directory_exe_toml() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let temp = TempDir::new().expect("tempdir");
        env::set_var("CHOPPER_CONFIG_DIR", temp.path());

        let path = create_alias("canonical").expect("create alias");

        assert_eq!(path, temp.path().join("canonical/exe.toml"));
        assert!(crate::find_config("canonical").is_some());
        env::remove_var("CHOPPER_CONFIG_DIR");
    }

    #[test]
    fn duplicate_and_rename_legacy_aliases_use_canonical_layout() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let temp = TempDir::new().expect("tempdir");
        let cfg = temp.path();
        fs::create_dir_all(cfg).expect("create cfg");
        fs::write(cfg.join("source.toml"), "exec = \"echo\"\n").expect("write source");
        env::set_var("CHOPPER_CONFIG_DIR", cfg);

        let duplicate_path = duplicate_alias("source", "copy").expect("duplicate alias");
        assert_eq!(duplicate_path, temp.path().join("copy/exe.toml"));
        assert!(duplicate_path.is_file());
        assert!(cfg.join("source.toml").is_file());
        assert!(cfg.join("source/exe.toml").is_file());

        let renamed_path = rename_alias("copy", "renamed").expect("rename alias");
        assert_eq!(renamed_path, temp.path().join("renamed/exe.toml"));
        assert!(renamed_path.is_file());
        assert!(!duplicate_path.exists());

        env::remove_var("CHOPPER_CONFIG_DIR");
    }

    #[test]
    fn duplicate_and_rename_canonical_aliases_use_target_alias_directory() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let temp = TempDir::new().expect("tempdir");
        env::set_var("CHOPPER_CONFIG_DIR", temp.path());

        create_alias("source").expect("create source alias");
        let duplicate_path = duplicate_alias("source", "copy").expect("duplicate alias");
        assert_eq!(duplicate_path, temp.path().join("copy/exe.toml"));
        assert!(duplicate_path.is_file());

        let renamed_path = rename_alias("copy", "renamed").expect("rename alias");
        assert_eq!(renamed_path, temp.path().join("renamed/exe.toml"));
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
        assert_eq!(path, temp.path().join("missing/exe.toml"));
        env::remove_var("CHOPPER_CONFIG_DIR");
    }
}
