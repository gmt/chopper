mod alias_admin;
mod alias_admin_parse;
mod alias_doc;
mod alias_validation;
mod arg_validation;
mod cache;
mod completion;
mod config_diagnostics;
mod env_util;
mod env_validation;
mod executor;
mod journal_broker_client;
mod journal_validation;
mod manifest;
mod parser;
mod path_validation;
mod reconcile;
mod rhai_api_catalog;
mod rhai_engine;
mod rhai_facade;
mod rhai_facade_validation;
mod rhai_wiring;
mod string_validation;
#[cfg(test)]
mod test_support;
mod tui;
mod tui_nvim;

use anyhow::{anyhow, Result};
use std::env;
use std::path::PathBuf;

pub(crate) fn config_dir() -> PathBuf {
    if let Some(override_path) = env_util::env_path_override("CHOPPER_CONFIG_DIR") {
        return override_path;
    }

    directories::ProjectDirs::from("", "", "chopper")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".chopper"))
}

pub(crate) fn find_config(name: &str) -> Option<PathBuf> {
    let cfg = config_dir();
    [
        cfg.join("aliases").join(format!("{name}.toml")),
        cfg.join(format!("{name}.toml")),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if let Some(action) = detect_builtin_action(&args) {
        run_builtin_action(action);
        return Ok(());
    }

    let invocation = parse_invocation(&args)?;

    let config_path = find_config(&invocation.alias);
    let manifest = match config_path {
        Some(path) => load_manifest(&invocation.alias, &path)?,
        None => {
            let exe = which::which(&invocation.alias)
                .unwrap_or_else(|_| PathBuf::from(&invocation.alias));
            manifest::Manifest::simple(exe)
        }
    };

    let patch = reconcile::maybe_reconcile(&manifest, &invocation.passthrough_args)?;
    let resolved = manifest.build_invocation(&invocation.passthrough_args, patch);
    executor::run(resolved)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BuiltinAction {
    Help,
    Version,
    PrintConfigDir,
    PrintCacheDir,
    Bashcomp,
    ListAliases,
    PrintExec(String),
    PrintBashcompMode(String),
    Complete(Vec<String>),
    Alias(Vec<String>),
    Tui(Vec<String>),
}

fn detect_builtin_action(args: &[String]) -> Option<BuiltinAction> {
    if !is_direct_invocation_executable(args) {
        return None;
    }
    if args.len() == 1 {
        return Some(BuiltinAction::Help);
    }

    let flag = args.get(1).map(String::as_str)?;

    // Two-argument builtins (chopper <flag>)
    if args.len() == 2 {
        match flag {
            "-h" | "--help" => return Some(BuiltinAction::Help),
            "-V" | "--version" => return Some(BuiltinAction::Version),
            "--print-config-dir" => return Some(BuiltinAction::PrintConfigDir),
            "--print-cache-dir" => return Some(BuiltinAction::PrintCacheDir),
            "--bashcomp" => return Some(BuiltinAction::Bashcomp),
            "--list-aliases" => return Some(BuiltinAction::ListAliases),
            _ => {}
        }
    }

    // Three-argument builtins (chopper <flag> <alias>)
    if args.len() == 3 {
        let alias = args[2].clone();
        match flag {
            "--print-exec" => return Some(BuiltinAction::PrintExec(alias)),
            "--print-bashcomp-mode" => return Some(BuiltinAction::PrintBashcompMode(alias)),
            _ => {}
        }
    }

    // Variable-length builtins (chopper --complete <alias> <cword> [--] <words...>)
    if flag == "--complete" && args.len() >= 3 {
        return Some(BuiltinAction::Complete(args[2..].to_vec()));
    }
    if flag == "--alias" {
        return Some(BuiltinAction::Alias(args[2..].to_vec()));
    }
    if flag == "--tui" {
        return Some(BuiltinAction::Tui(args[2..].to_vec()));
    }

    None
}

fn run_builtin_action(action: BuiltinAction) {
    match action {
        BuiltinAction::Help => {
            println!("Usage:");
            println!("  chopper <alias> [args...]");
            println!("  chopper <alias> -- [args...]");
            println!("  <symlinked-alias> [args...]");
            println!();
            println!("Built-ins:");
            println!("  -h, --help                   Show this help");
            println!("  -V, --version                Show version");
            println!("  --print-config-dir           Print resolved config root");
            println!("  --print-cache-dir            Print resolved cache root");
            println!("  --bashcomp                   Emit bash completion script");
            println!("  --list-aliases               List configured aliases");
            println!("  --print-exec <alias>         Print resolved exec path for alias");
            println!("  --print-bashcomp-mode <alias> Print bashcomp mode for alias");
            println!("  --complete <alias> <cword> [--] <words...>");
            println!("                               Run Rhai completion for alias");
            println!("  --alias <subcommand> [...]   Alias lifecycle management");
            println!("  --tui                        Open interactive terminal UI");
            println!();
            println!("Environment overrides:");
            println!("  CHOPPER_CONFIG_DIR=/path/to/config-root");
            println!("  CHOPPER_CACHE_DIR=/path/to/cache-root");
            println!("  CHOPPER_DISABLE_CACHE=<truthy>   # 1,true,yes,on");
            println!("  CHOPPER_DISABLE_RECONCILE=<truthy>   # 1,true,yes,on");
        }
        BuiltinAction::Version => {
            println!("chopper {}", env!("CARGO_PKG_VERSION"));
        }
        BuiltinAction::PrintConfigDir => {
            println!("{}", config_dir().display());
        }
        BuiltinAction::PrintCacheDir => {
            println!("{}", cache::cache_dir().display());
        }
        BuiltinAction::Bashcomp => {
            print!("{}", include_str!("bashcomp.bash"));
        }
        BuiltinAction::ListAliases => {
            emit_config_scan_warnings();
            run_list_aliases();
        }
        BuiltinAction::PrintExec(alias) => {
            std::process::exit(run_print_exec(&alias));
        }
        BuiltinAction::PrintBashcompMode(alias) => {
            std::process::exit(run_print_bashcomp_mode(&alias));
        }
        BuiltinAction::Complete(raw_args) => {
            std::process::exit(run_complete_builtin(&raw_args));
        }
        BuiltinAction::Alias(raw_args) => {
            emit_config_scan_warnings();
            std::process::exit(alias_admin::run_alias_action(&raw_args));
        }
        BuiltinAction::Tui(raw_args) => {
            let options = match parse_tui_options(&raw_args) {
                Ok(options) => options,
                Err(err) => {
                    eprintln!("{err}");
                    std::process::exit(2);
                }
            };
            std::process::exit(tui::run_tui(options));
        }
    }
}

fn parse_tui_options(raw_args: &[String]) -> Result<tui::TuiOptions> {
    if let Some(arg) = raw_args.first() {
        return Err(anyhow!(
            "unknown --tui option `{arg}`; --tui currently accepts no options"
        ));
    }
    Ok(tui::TuiOptions)
}

fn run_list_aliases() {
    let cfg = config_dir();
    let mut aliases = std::collections::BTreeSet::new();

    // Scan aliases/ subdirectory
    let aliases_dir = cfg.join("aliases");
    if let Ok(entries) = std::fs::read_dir(&aliases_dir) {
        for entry in entries.flatten() {
            if let Some(name) = alias_name_from_dir_entry(&entry) {
                aliases.insert(name);
            }
        }
    }

    // Scan config root
    if let Ok(entries) = std::fs::read_dir(&cfg) {
        for entry in entries.flatten() {
            // Skip the aliases/ subdirectory itself
            if entry.file_name() == "aliases" {
                continue;
            }
            if let Some(name) = alias_name_from_dir_entry(&entry) {
                aliases.insert(name);
            }
        }
    }

    for alias in &aliases {
        println!("{alias}");
    }
}

fn emit_config_scan_warnings() {
    let cfg = config_dir();
    let mut warnings = config_diagnostics::scan_extension_warnings(&cfg);
    warnings.sort();
    warnings.dedup();
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
}

fn alias_name_from_dir_entry(entry: &std::fs::DirEntry) -> Option<String> {
    let path = entry.path();
    // Accept regular files and symlinks that resolve to regular files.
    if !path.is_file() {
        return None;
    }
    let name = entry.file_name();
    let name = name.to_str()?;
    let alias = name.strip_suffix(".toml")?;
    if alias.is_empty() {
        return None;
    }
    Some(alias.to_string())
}

fn run_print_exec(alias: &str) -> i32 {
    let config_path = find_config(alias);
    let manifest = match config_path {
        Some(path) => match load_manifest(alias, &path) {
            Ok(m) => m,
            Err(_) => return 1,
        },
        None => {
            // No config; try PATH lookup like normal execution
            match which::which(alias) {
                Ok(exe) => manifest::Manifest::simple(exe),
                Err(_) => return 1,
            }
        }
    };
    println!("{}", manifest.exec.display());
    0
}

fn run_print_bashcomp_mode(alias: &str) -> i32 {
    let config_path = find_config(alias);
    let manifest = match config_path {
        Some(path) => match load_manifest(alias, &path) {
            Ok(m) => m,
            Err(_) => {
                println!("normal");
                return 0;
            }
        },
        None => {
            // No config; default to normal
            println!("normal");
            return 0;
        }
    };

    match &manifest.bashcomp {
        Some(bc) if bc.disabled => println!("disabled"),
        Some(bc) if bc.script.is_some() => println!("custom"),
        Some(bc) if bc.rhai_script.is_some() => println!("rhai"),
        Some(bc) if bc.passthrough => println!("passthrough"),
        _ => println!("normal"),
    }
    0
}

fn run_complete_builtin(raw_args: &[String]) -> i32 {
    // raw_args = [<alias>, <cword>, [--], <word0>, ...]
    if raw_args.len() < 2 {
        eprintln!("usage: chopper --complete <alias> <cword> [--] <words...>");
        return 1;
    }

    let alias = &raw_args[0];
    let cword: usize = match raw_args[1].parse() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("invalid cword: {}", raw_args[1]);
            return 1;
        }
    };

    let words_start = if raw_args.get(2).map(String::as_str) == Some("--") {
        3
    } else {
        2
    };
    let words: Vec<String> = raw_args[words_start..].to_vec();

    let config_path = find_config(alias);
    let manifest = match config_path {
        Some(path) => match load_manifest(alias, &path) {
            Ok(m) => m,
            Err(_) => return 1,
        },
        None => return 1,
    };

    match completion::run_complete(&manifest, &words, cword) {
        Ok(candidates) => {
            for candidate in candidates {
                println!("{candidate}");
            }
            0
        }
        Err(_) => 1,
    }
}

fn load_manifest(alias: &str, path: &std::path::Path) -> Result<manifest::Manifest> {
    if !cache_enabled() {
        return parser::parse(path);
    }

    let fingerprint = cache::source_fingerprint(path)?;
    if let Some(cached) = cache::load(alias, &fingerprint) {
        return Ok(cached);
    }

    let manifest = parser::parse(path)?;
    cache::store(alias, &fingerprint, &manifest)?;
    Ok(manifest)
}

fn cache_enabled() -> bool {
    !env_util::env_flag_enabled("CHOPPER_DISABLE_CACHE")
}

#[derive(Debug, PartialEq, Eq)]
struct InvocationInput {
    alias: String,
    passthrough_args: Vec<String>,
}

fn parse_invocation(args: &[String]) -> Result<InvocationInput> {
    let exe_name = invocation_executable_name(args);

    if is_direct_invocation_executable(args) {
        if args.len() < 2 {
            return Err(anyhow!(
                "missing alias name; use `chopper <alias> [args...]` or `chopper --help`"
            ));
        }
        let alias = args[1].clone();
        validate_alias_name(&alias)?;
        let passthrough_args = normalize_passthrough(&args[2..]);
        validate_passthrough_args(&passthrough_args)?;
        Ok(InvocationInput {
            alias,
            passthrough_args,
        })
    } else {
        validate_alias_name(&exe_name)?;
        let passthrough_args = normalize_passthrough(&args[1..]);
        validate_passthrough_args(&passthrough_args)?;
        Ok(InvocationInput {
            alias: exe_name,
            passthrough_args,
        })
    }
}

fn validate_passthrough_args(args: &[String]) -> Result<()> {
    for arg in args {
        if matches!(
            crate::arg_validation::validate_arg_value(arg),
            Err(crate::arg_validation::ArgViolation::ContainsNul)
        ) {
            return Err(anyhow!("runtime arguments cannot contain NUL bytes"));
        }
    }
    Ok(())
}

fn validate_alias_name(alias: &str) -> Result<()> {
    use crate::alias_validation::AliasViolation;

    match crate::alias_validation::validate_alias_identifier(alias) {
        Ok(()) => Ok(()),
        Err(AliasViolation::Empty) => Err(anyhow!("alias name cannot be empty")),
        Err(AliasViolation::ContainsNul) => {
            Err(anyhow!("alias name cannot contain NUL bytes"))
        }
        Err(AliasViolation::IsSeparator) => Err(anyhow!(
            "alias name cannot be `--`; expected `chopper <alias> -- [args...]`"
        )),
        Err(AliasViolation::StartsWithDash) => Err(anyhow!(
            "alias name cannot start with `-`; choose a non-flag alias name"
        )),
        Err(AliasViolation::ContainsWhitespace) => {
            Err(anyhow!("alias name cannot contain whitespace"))
        }
        Err(AliasViolation::IsDotToken) => Err(anyhow!("alias name cannot be `.` or `..`")),
        Err(AliasViolation::ContainsPathSeparator) => Err(anyhow!(
            "alias name cannot contain path separators; use symlink mode or command PATH resolution instead"
        )),
    }
}

fn invocation_executable_name(args: &[String]) -> String {
    let raw = args.first().map(String::as_str).unwrap_or("chopper");
    let basename = if raw.contains('/') {
        raw.trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("chopper")
    } else {
        raw
    };
    let basename = if basename.is_empty() || basename == "." || basename == ".." {
        "chopper"
    } else {
        basename
    };

    PathBuf::from(basename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("chopper")
        .to_string()
}

fn is_direct_invocation_executable(args: &[String]) -> bool {
    is_direct_chopper_name(&invocation_executable_name(args))
}

fn is_direct_chopper_name(exe_name: &str) -> bool {
    exe_name.eq_ignore_ascii_case("chopper")
}

fn normalize_passthrough(args: &[String]) -> Vec<String> {
    if args.first().map(String::as_str) == Some("--") {
        args[1..].to_vec()
    } else {
        args.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cache_enabled, config_dir, detect_builtin_action, find_config, parse_invocation,
        parse_tui_options, validate_alias_name, BuiltinAction,
    };
    use crate::test_support::ENV_LOCK;
    use std::env;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn supports_direct_invocation_mode() {
        let invocation = parse_invocation(&[
            "chopper".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");
        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn supports_symlink_invocation_mode() {
        let invocation = parse_invocation(&[
            "kubectl-prod".to_string(),
            "get".to_string(),
            "pods".to_string(),
        ])
        .expect("valid invocation");
        assert_eq!(invocation.alias, "kubectl-prod");
        assert_eq!(invocation.passthrough_args, vec!["get", "pods"]);
    }

    #[test]
    fn strips_double_dash_separator() {
        let invocation = parse_invocation(&[
            "chopper".to_string(),
            "kpods".to_string(),
            "--".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn rejects_invalid_alias_identifiers() {
        assert!(validate_alias_name("-alias").is_err());
        assert!(validate_alias_name("bad alias").is_err());
        assert!(validate_alias_name("bad/alias").is_err());
    }

    #[test]
    fn cache_enabled_by_default() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_CACHE");
        assert!(cache_enabled());
    }

    #[test]
    fn cache_disable_flag_truthy_disables_cache() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::set_var("CHOPPER_DISABLE_CACHE", "true");
        assert!(!cache_enabled());
        env::remove_var("CHOPPER_DISABLE_CACHE");
    }

    #[test]
    fn config_dir_honors_chopper_override() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::set_var("CHOPPER_CONFIG_DIR", "/tmp/chopper-config-override");
        assert_eq!(config_dir(), PathBuf::from("/tmp/chopper-config-override"));
        env::remove_var("CHOPPER_CONFIG_DIR");
    }

    #[test]
    fn find_config_accepts_symlinked_file_candidates() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let temp = TempDir::new().expect("create temp config dir");
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir).expect("create aliases dir");
        let target = temp.path().join("target.toml");
        fs::write(&target, "exec = \"echo\"\n").expect("write symlink target");
        let alias_symlink = aliases_dir.join("demo.toml");
        symlink(&target, &alias_symlink).expect("create alias symlink");

        env::set_var("CHOPPER_CONFIG_DIR", temp.path());
        let found = find_config("demo").expect("expected symlinked config");
        assert_eq!(found, alias_symlink);
        env::remove_var("CHOPPER_CONFIG_DIR");
    }

    #[test]
    fn detects_common_builtins_in_direct_mode() {
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "--list-aliases".into()]),
            Some(BuiltinAction::ListAliases)
        );
    }

    #[test]
    fn does_not_detect_builtins_in_symlink_mode() {
        assert_eq!(
            detect_builtin_action(&["myalias".into(), "--help".into()]),
            None
        );
        assert_eq!(
            detect_builtin_action(&["myalias".into(), "--version".into()]),
            None
        );
    }

    #[test]
    fn detects_tui_and_complete_actions() {
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "--tui".into()]),
            Some(BuiltinAction::Tui(Vec::new()))
        );
        assert_eq!(
            detect_builtin_action(&[
                "chopper".into(),
                "--complete".into(),
                "kpods".into(),
                "1".into(),
                "--".into(),
                "kpods".into(),
            ]),
            Some(BuiltinAction::Complete(vec![
                "kpods".into(),
                "1".into(),
                "--".into(),
                "kpods".into(),
            ]))
        );
    }

    #[test]
    fn parse_tui_options_rejects_all_flags() {
        assert!(parse_tui_options(&[]).is_ok());
        let err = parse_tui_options(&["--tmux=off".into()]).expect_err("tmux flags removed");
        assert!(err.to_string().contains("no options"), "{err}");
    }
}
