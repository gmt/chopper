mod cache;
mod env_util;
mod executor;
mod manifest;
mod parser;
mod reconcile;
#[cfg(test)]
mod test_support;

use anyhow::{anyhow, Result};
use std::env;
use std::path::PathBuf;

fn config_dir() -> PathBuf {
    if let Some(override_path) = env_util::env_path_override("CHOPPER_CONFIG_DIR") {
        return override_path;
    }

    directories::ProjectDirs::from("", "", "chopper")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".chopper"))
}

fn find_config(name: &str) -> Option<PathBuf> {
    let cfg = config_dir();
    [
        cfg.join("aliases").join(format!("{name}.toml")),
        cfg.join(format!("{name}.toml")),
        cfg.join(name),
        cfg.join(format!("{name}.conf")),
        cfg.join(format!("{name}.rhai")),
    ]
    .into_iter()
    .find(|path| path.exists())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinAction {
    Help,
    Version,
}

fn detect_builtin_action(args: &[String]) -> Option<BuiltinAction> {
    let exe_name = invocation_executable_name(args);
    if exe_name != "chopper" {
        return None;
    }

    match args.get(1).map(String::as_str) {
        Some("-h" | "--help") => Some(BuiltinAction::Help),
        Some("-V" | "--version") => Some(BuiltinAction::Version),
        _ => None,
    }
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
            println!("  -h, --help       Show this help");
            println!("  -V, --version    Show version");
            println!();
            println!("Environment overrides:");
            println!("  CHOPPER_CONFIG_DIR=/path/to/config-root");
            println!("  CHOPPER_CACHE_DIR=/path/to/cache-root");
            println!("  CHOPPER_DISABLE_CACHE=1");
            println!("  CHOPPER_DISABLE_RECONCILE=1");
        }
        BuiltinAction::Version => {
            println!("chopper {}", env!("CARGO_PKG_VERSION"));
        }
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

    if exe_name == "chopper" {
        if args.len() < 2 {
            eprintln!("Usage: symlink to chopper with alias name, or chopper <alias>");
            eprintln!("  chopper <alias> [args...]");
            std::process::exit(1);
        }
        let alias = args[1].clone();
        if alias.trim().is_empty() {
            return Err(anyhow!("alias name cannot be empty"));
        }
        if alias == "--" {
            return Err(anyhow!(
                "alias name cannot be `--`; expected `chopper <alias> -- [args...]`"
            ));
        }
        let passthrough_args = normalize_passthrough(&args[2..]);
        Ok(InvocationInput {
            alias,
            passthrough_args,
        })
    } else {
        Ok(InvocationInput {
            alias: exe_name,
            passthrough_args: normalize_passthrough(&args[1..]),
        })
    }
}

fn invocation_executable_name(args: &[String]) -> String {
    PathBuf::from(
        args.first()
            .cloned()
            .unwrap_or_else(|| "chopper".to_string()),
    )
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("chopper")
    .to_string()
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
        cache_enabled, config_dir, detect_builtin_action, parse_invocation, BuiltinAction,
    };
    use crate::test_support::ENV_LOCK;
    use std::env;
    use std::path::PathBuf;

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
    fn strips_double_dash_separator_for_direct_invocation() {
        let invocation = parse_invocation(&[
            "chopper".to_string(),
            "kpods".to_string(),
            "--".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn strips_double_dash_separator_for_symlink_invocation() {
        let invocation = parse_invocation(&[
            "kubectl-prod".to_string(),
            "--".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kubectl-prod");
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
    fn rejects_separator_as_alias_name() {
        let err = parse_invocation(&[
            "chopper".to_string(),
            "--".to_string(),
            "runtime".to_string(),
        ])
        .expect_err("separator cannot be alias");

        assert!(err.to_string().contains("alias name cannot be `--`"));
    }

    #[test]
    fn cache_enabled_by_default() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_CACHE");
        assert!(cache_enabled());
    }

    #[test]
    fn cache_can_be_disabled_via_env() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::set_var("CHOPPER_DISABLE_CACHE", "true");
        assert!(!cache_enabled());
        env::set_var("CHOPPER_DISABLE_CACHE", "1");
        assert!(!cache_enabled());
        env::set_var("CHOPPER_DISABLE_CACHE", "yes");
        assert!(!cache_enabled());
        env::remove_var("CHOPPER_DISABLE_CACHE");
    }

    #[test]
    fn config_dir_honors_chopper_override() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::set_var("CHOPPER_CONFIG_DIR", "/tmp/chopper-config-override");
        let path = config_dir();
        assert_eq!(path, PathBuf::from("/tmp/chopper-config-override"));
        env::remove_var("CHOPPER_CONFIG_DIR");
    }

    #[test]
    fn empty_config_override_falls_back_to_default_logic() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::set_var("CHOPPER_CONFIG_DIR", "   ");
        let path = config_dir();
        assert_ne!(path, PathBuf::from("   "));
        env::remove_var("CHOPPER_CONFIG_DIR");
    }

    #[test]
    fn detects_help_action_only_for_direct_chopper_invocation() {
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["alias-symlink".into(), "--help".into()]),
            None
        );
    }

    #[test]
    fn detects_version_action_for_direct_chopper_invocation() {
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
    }
}
