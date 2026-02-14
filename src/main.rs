mod alias_validation;
mod arg_validation;
mod cache;
mod env_util;
mod env_validation;
mod executor;
mod journal_validation;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinAction {
    Help,
    Version,
    PrintConfigDir,
    PrintCacheDir,
}

fn detect_builtin_action(args: &[String]) -> Option<BuiltinAction> {
    if !is_direct_invocation_executable(args) {
        return None;
    }
    if args.len() == 1 {
        return Some(BuiltinAction::Help);
    }
    if args.len() != 2 {
        return None;
    }

    match args.get(1).map(String::as_str) {
        Some("-h" | "--help") => Some(BuiltinAction::Help),
        Some("-V" | "--version") => Some(BuiltinAction::Version),
        Some("--print-config-dir") => Some(BuiltinAction::PrintConfigDir),
        Some("--print-cache-dir") => Some(BuiltinAction::PrintCacheDir),
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
            println!("  --print-config-dir  Print resolved config root");
            println!("  --print-cache-dir   Print resolved cache root");
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
        BuiltinAction::PrintConfigDir => {
            println!("{}", config_dir().display());
        }
        BuiltinAction::PrintCacheDir => {
            println!("{}", cache::cache_dir().display());
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
    } else if looks_like_windows_invocation_path(raw) {
        raw.trim_end_matches(['/', '\\'])
            .rsplit(['/', '\\'])
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
    if is_direct_chopper_name(&invocation_executable_name(args)) {
        return true;
    }

    windows_relative_basename(args.first().map(String::as_str).unwrap_or("chopper"))
        .map(is_direct_chopper_name)
        .unwrap_or(false)
}

fn looks_like_windows_invocation_path(raw: &str) -> bool {
    raw.starts_with("\\\\") || has_windows_drive_prefix(raw)
}

fn has_windows_drive_prefix(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn windows_relative_basename(raw: &str) -> Option<&str> {
    if !(raw.starts_with(".\\") || raw.starts_with("..\\")) {
        return None;
    }

    let trimmed = raw.trim_end_matches(['/', '\\']);
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
}

fn is_direct_chopper_name(exe_name: &str) -> bool {
    exe_name.eq_ignore_ascii_case("chopper")
        || exe_name.eq_ignore_ascii_case("chopper.exe")
        || exe_name.eq_ignore_ascii_case("chopper.com")
        || exe_name.eq_ignore_ascii_case("chopper.cmd")
        || exe_name.eq_ignore_ascii_case("chopper.bat")
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
        cache_enabled, config_dir, detect_builtin_action, find_config,
        is_direct_invocation_executable, parse_invocation, validate_alias_name,
        windows_relative_basename, BuiltinAction,
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
    fn direct_invocation_separator_strips_only_leading_marker() {
        let invocation = parse_invocation(&[
            "chopper".to_string(),
            "kpods".to_string(),
            "--".to_string(),
            "--".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--", "--tail=100"]);
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
    fn symlink_invocation_separator_strips_only_leading_marker() {
        let invocation = parse_invocation(&[
            "kubectl-prod".to_string(),
            "--".to_string(),
            "--".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kubectl-prod");
        assert_eq!(invocation.passthrough_args, vec!["--", "--tail=100"]);
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
    fn preserves_symlink_aliases_containing_dots() {
        let invocation = parse_invocation(&["kubectl.prod".to_string(), "get".to_string()])
            .expect("valid invocation");

        assert_eq!(invocation.alias, "kubectl.prod");
        assert_eq!(invocation.passthrough_args, vec!["get"]);
    }

    #[test]
    fn symlink_invocation_uses_executable_basename_from_path() {
        let invocation = parse_invocation(&[
            "/tmp/bin/kubectl.prod".to_string(),
            "get".to_string(),
            "pods".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kubectl.prod");
        assert_eq!(invocation.passthrough_args, vec!["get", "pods"]);
    }

    #[test]
    fn symlink_invocation_uses_windows_style_basename_from_path() {
        let invocation = parse_invocation(&[
            "C:\\tools\\kubectl.prod".to_string(),
            "get".to_string(),
            "pods".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kubectl.prod");
        assert_eq!(invocation.passthrough_args, vec!["get", "pods"]);
    }

    #[test]
    fn symlink_invocation_uses_unc_windows_basename_from_path() {
        let invocation = parse_invocation(&[
            "\\\\server\\tools\\kubectl.prod".to_string(),
            "get".to_string(),
            "pods".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kubectl.prod");
        assert_eq!(invocation.passthrough_args, vec!["get", "pods"]);
    }

    #[test]
    fn parse_invocation_treats_chopper_exe_as_direct_mode() {
        let invocation = parse_invocation(&[
            "/tmp/bin/chopper.exe".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_chopper_cmd_as_direct_mode() {
        let invocation = parse_invocation(&[
            "/tmp/bin/chopper.cmd".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_chopper_com_as_direct_mode() {
        let invocation = parse_invocation(&[
            "/tmp/bin/chopper.com".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_uppercase_chopper_com_as_direct_mode() {
        let invocation = parse_invocation(&[
            "/tmp/bin/CHOPPER.COM".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_uppercase_chopper_cmd_as_direct_mode() {
        let invocation = parse_invocation(&[
            "/tmp/bin/CHOPPER.CMD".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_uppercase_chopper_bat_as_direct_mode() {
        let invocation = parse_invocation(&[
            "/tmp/bin/CHOPPER.BAT".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_chopper_bat_as_direct_mode() {
        let invocation = parse_invocation(&[
            "/tmp/bin/chopper.bat".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_uppercase_chopper_name_as_direct_mode() {
        let invocation = parse_invocation(&[
            "CHOPPER".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_windows_style_chopper_exe_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "C:\\tools\\chopper.exe".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_windows_drive_forward_slash_chopper_cmd_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "C:/tools/CHOPPER.CMD".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_unix_relative_chopper_cmd_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "./CHOPPER.CMD".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_unix_parent_relative_chopper_bat_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "../CHOPPER.BAT".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_drive_windows_chopper_cmd_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "C:\\tools\\CHOPPER.CMD".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_drive_windows_chopper_bat_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "D:\\bin\\CHOPPER.BAT".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_drive_windows_chopper_com_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "E:\\tools\\CHOPPER.COM".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_unc_windows_chopper_cmd_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "\\\\server\\tools\\CHOPPER.CMD".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_unc_forward_slash_chopper_com_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "//server/tools/CHOPPER.COM".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_unc_windows_chopper_com_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "\\\\server\\tools\\CHOPPER.COM".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_windows_relative_chopper_exe_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            ".\\chopper.exe".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_windows_relative_chopper_cmd_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            ".\\chopper.cmd".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_windows_relative_chopper_bat_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            ".\\chopper.bat".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_windows_relative_chopper_com_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            ".\\chopper.com".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_windows_relative_uppercase_chopper_bat_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            ".\\CHOPPER.BAT".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_windows_relative_uppercase_chopper_com_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            ".\\CHOPPER.COM".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_parent_windows_relative_chopper_exe_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "..\\CHOPPER.EXE".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_parent_windows_relative_chopper_cmd_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "..\\CHOPPER.CMD".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_parent_windows_relative_chopper_bat_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "..\\CHOPPER.BAT".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
    }

    #[test]
    fn parse_invocation_treats_parent_windows_relative_chopper_com_path_as_direct_mode() {
        let invocation = parse_invocation(&[
            "..\\CHOPPER.COM".to_string(),
            "kpods".to_string(),
            "--tail=100".to_string(),
        ])
        .expect("valid invocation");

        assert_eq!(invocation.alias, "kpods");
        assert_eq!(invocation.passthrough_args, vec!["--tail=100"]);
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
    fn rejects_alias_with_path_separators() {
        let err = validate_alias_name("foo/bar").expect_err("path separators are invalid");
        assert!(err.to_string().contains("path separators"));
    }

    #[test]
    fn rejects_alias_with_nul_bytes() {
        let err = validate_alias_name("bad\0alias").expect_err("nul bytes are invalid");
        assert!(err.to_string().contains("cannot contain NUL bytes"));
    }

    #[test]
    fn rejects_dot_alias_tokens() {
        assert!(validate_alias_name(".").is_err());
        assert!(validate_alias_name("..").is_err());
    }

    #[test]
    fn parse_invocation_rejects_alias_with_nul_bytes() {
        let err = parse_invocation(&["chopper".to_string(), "bad\0alias".to_string()])
            .expect_err("alias with nul should be invalid");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_rejects_empty_argv_shape() {
        let err = parse_invocation(&[]).expect_err("empty argv should be invalid");
        assert!(err.to_string().contains("missing alias name"), "{err}");
    }

    #[test]
    fn parse_invocation_rejects_symlink_alias_with_nul_bytes() {
        let err = parse_invocation(&["bad\0alias".to_string()])
            .expect_err("symlink alias with nul should be invalid");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
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
            detect_builtin_action(&["CHOPPER.COM".into(), "-h".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.COM".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.CMD".into(), "-h".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.cmd".into(), "-h".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.bat".into(), "-h".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.cmd".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.BAT".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.exe".into(), "-h".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.EXE".into(), "-h".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER".into(), "-h".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&[".\\chopper.exe".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&[".\\CHOPPER.CMD".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&[".\\CHOPPER.BAT".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["..\\CHOPPER.EXE".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["..\\CHOPPER.CMD".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["C:\\tools\\chopper.exe".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["./CHOPPER.COM".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["D:\\bin\\CHOPPER.BAT".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["C:/tools/CHOPPER.CMD".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["\\\\server\\tools\\CHOPPER.CMD".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.exe".into(), "--help".into()]),
            Some(BuiltinAction::Help)
        );
        assert_eq!(
            detect_builtin_action(&["chopper".into()]),
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
            detect_builtin_action(&["CHOPPER.COM".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.COM".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.com".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.CMD".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.BAT".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.cmd".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.bat".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.bat".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.cmd".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.exe".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.EXE".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&[".\\CHOPPER.EXE".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&[".\\CHOPPER.COM".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["..\\chopper.bat".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["..\\CHOPPER.COM".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["\\\\server\\tools\\CHOPPER.BAT".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["E:\\tools\\CHOPPER.COM".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["//server/tools/CHOPPER.BAT".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.exe".into(), "--version".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["../CHOPPER.CMD".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "-V".into()]),
            Some(BuiltinAction::Version)
        );
    }

    #[test]
    fn detects_print_path_actions_for_direct_chopper_invocation() {
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "--print-config-dir".into()]),
            Some(BuiltinAction::PrintConfigDir)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.COM".into(), "--print-cache-dir".into()]),
            Some(BuiltinAction::PrintCacheDir)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.COM".into(), "--print-config-dir".into()]),
            Some(BuiltinAction::PrintConfigDir)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.CMD".into(), "--print-config-dir".into()]),
            Some(BuiltinAction::PrintConfigDir)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.cmd".into(), "--print-cache-dir".into()]),
            Some(BuiltinAction::PrintCacheDir)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.bat".into(), "--print-cache-dir".into()]),
            Some(BuiltinAction::PrintCacheDir)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.BAT".into(), "--print-cache-dir".into()]),
            Some(BuiltinAction::PrintCacheDir)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.BAT".into(), "--print-config-dir".into()]),
            Some(BuiltinAction::PrintConfigDir)
        );
        assert_eq!(
            detect_builtin_action(&["/tmp/chopper.exe".into(), "--print-cache-dir".into()]),
            Some(BuiltinAction::PrintCacheDir)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER".into(), "--print-config-dir".into()]),
            Some(BuiltinAction::PrintConfigDir)
        );
        assert_eq!(
            detect_builtin_action(&["C:\\tools\\chopper.exe".into(), "--print-config-dir".into()]),
            Some(BuiltinAction::PrintConfigDir)
        );
        assert_eq!(
            detect_builtin_action(&["./CHOPPER.BAT".into(), "--print-config-dir".into()]),
            Some(BuiltinAction::PrintConfigDir)
        );
        assert_eq!(
            detect_builtin_action(&["D:\\bin\\CHOPPER.CMD".into(), "--print-cache-dir".into()]),
            Some(BuiltinAction::PrintCacheDir)
        );
        assert_eq!(
            detect_builtin_action(&[
                "//server/tools/CHOPPER.COM".into(),
                "--print-config-dir".into()
            ]),
            Some(BuiltinAction::PrintConfigDir)
        );
        assert_eq!(
            detect_builtin_action(&[
                "\\\\server\\tools\\CHOPPER.EXE".into(),
                "--print-cache-dir".into()
            ]),
            Some(BuiltinAction::PrintCacheDir)
        );
        assert_eq!(
            detect_builtin_action(&[".\\CHOPPER.COM".into(), "--print-config-dir".into()]),
            Some(BuiltinAction::PrintConfigDir)
        );
        assert_eq!(
            detect_builtin_action(&["..\\CHOPPER.BAT".into(), "--print-cache-dir".into()]),
            Some(BuiltinAction::PrintCacheDir)
        );
        assert_eq!(
            detect_builtin_action(&[
                "\\\\server\\tools\\CHOPPER.COM".into(),
                "--print-config-dir".into()
            ]),
            Some(BuiltinAction::PrintConfigDir)
        );
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "--print-cache-dir".into()]),
            Some(BuiltinAction::PrintCacheDir)
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER".into(), "--print-cache-dir".into()]),
            Some(BuiltinAction::PrintCacheDir)
        );
        assert_eq!(
            detect_builtin_action(&["symlink-alias".into(), "--print-config-dir".into()]),
            None
        );
    }

    #[test]
    fn builtin_detection_requires_exact_argument_shape() {
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "--help".into(), "extra".into()]),
            None
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.COM".into(), "--help".into(), "extra".into()]),
            None
        );
        assert_eq!(
            detect_builtin_action(&["CHOPPER.BAT".into(), "--help".into(), "extra".into()]),
            None
        );
        assert_eq!(
            detect_builtin_action(&[
                "CHOPPER.EXE".into(),
                "--print-config-dir".into(),
                "extra".into()
            ]),
            None
        );
        assert_eq!(
            detect_builtin_action(&["chopper".into(), "--version".into(), "extra".into()]),
            None
        );
        assert_eq!(
            detect_builtin_action(&[".\\chopper.exe".into(), "-h".into(), "extra".into()]),
            None
        );
        assert_eq!(
            detect_builtin_action(&[
                "/tmp/chopper.exe".into(),
                "--print-cache-dir".into(),
                "extra".into()
            ]),
            None
        );
        assert_eq!(
            detect_builtin_action(&[".\\CHOPPER.BAT".into(), "--help".into(), "extra".into()]),
            None
        );
        assert_eq!(
            detect_builtin_action(&["..\\CHOPPER.COM".into(), "--help".into(), "extra".into()]),
            None
        );
        assert_eq!(
            detect_builtin_action(&[
                "\\\\server\\tools\\CHOPPER.CMD".into(),
                "--help".into(),
                "extra".into()
            ]),
            None
        );
        assert_eq!(
            detect_builtin_action(&[
                "C:\\tools\\CHOPPER.COM".into(),
                "--help".into(),
                "extra".into()
            ]),
            None
        );
        assert_eq!(
            detect_builtin_action(&["./CHOPPER.BAT".into(), "--help".into(), "extra".into()]),
            None
        );
        assert_eq!(
            detect_builtin_action(&[
                "C:/tools/CHOPPER.COM".into(),
                "--help".into(),
                "extra".into()
            ]),
            None
        );
    }

    #[test]
    fn windows_relative_basename_extracts_only_dot_and_parent_prefixes() {
        assert_eq!(
            windows_relative_basename(".\\chopper.exe"),
            Some("chopper.exe")
        );
        assert_eq!(
            windows_relative_basename("..\\CHOPPER.EXE"),
            Some("CHOPPER.EXE")
        );
        assert_eq!(
            windows_relative_basename(".\\nested\\chopper.exe"),
            Some("chopper.exe")
        );
        assert_eq!(
            windows_relative_basename("..\\nested\\CHOPPER.EXE"),
            Some("CHOPPER.EXE")
        );
        assert_eq!(
            windows_relative_basename(".\\nested\\chopper.com"),
            Some("chopper.com")
        );
        assert_eq!(
            windows_relative_basename("..\\nested\\CHOPPER.CMD"),
            Some("CHOPPER.CMD")
        );
        assert_eq!(
            windows_relative_basename("..\\nested\\CHOPPER.BAT"),
            Some("CHOPPER.BAT")
        );
        assert_eq!(
            windows_relative_basename("..\\nested\\CHOPPER.COM"),
            Some("CHOPPER.COM")
        );
    }

    #[test]
    fn windows_relative_basename_does_not_match_other_path_shapes() {
        assert_eq!(windows_relative_basename("bad\\alias"), None);
        assert_eq!(
            windows_relative_basename("\\\\server\\tools\\chopper.exe"),
            None
        );
        assert_eq!(windows_relative_basename("C:\\tools\\chopper.exe"), None);
        assert_eq!(windows_relative_basename("chopper.exe"), None);
    }

    #[test]
    fn direct_executable_detection_is_specific_to_chopper_names() {
        assert!(is_direct_invocation_executable(&[".\\chopper.exe".into()]));
        assert!(is_direct_invocation_executable(&["..\\CHOPPER.EXE".into()]));
        assert!(is_direct_invocation_executable(&["CHOPPER".into()]));

        assert!(!is_direct_invocation_executable(&[
            ".\\not-chopper.exe".into()
        ]));
        assert!(!is_direct_invocation_executable(&["..\\alias".into()]));
    }

    #[test]
    fn parse_invocation_rejects_missing_alias_without_exiting_process() {
        let err = parse_invocation(&["chopper".into()]).expect_err("missing alias is invalid");
        assert!(err.to_string().contains("missing alias name"));
    }

    #[test]
    fn parse_invocation_rejects_direct_passthrough_args_with_nul_bytes() {
        let err = parse_invocation(&["chopper".into(), "demo".into(), "bad\0arg".into()])
            .expect_err("nul bytes should be rejected");
        assert!(
            err.to_string()
                .contains("runtime arguments cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_rejects_symlink_passthrough_args_with_nul_bytes() {
        let err = parse_invocation(&["demo".into(), "bad\0arg".into()])
            .expect_err("nul bytes should be rejected");
        assert!(
            err.to_string()
                .contains("runtime arguments cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_rejects_direct_passthrough_nul_after_separator() {
        let err = parse_invocation(&[
            "chopper".into(),
            "demo".into(),
            "--".into(),
            "bad\0arg".into(),
        ])
        .expect_err("nul bytes should be rejected even after `--`");
        assert!(
            err.to_string()
                .contains("runtime arguments cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_rejects_symlink_passthrough_nul_after_separator() {
        let err = parse_invocation(&["demo".into(), "--".into(), "bad\0arg".into()])
            .expect_err("nul bytes should be rejected even after `--`");
        assert!(
            err.to_string()
                .contains("runtime arguments cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_rejects_dash_prefixed_symlink_alias() {
        let err = parse_invocation(&["-bad-alias".into()])
            .expect_err("dash-prefixed symlink alias should be rejected");
        assert!(
            err.to_string().contains("alias name cannot start with `-`"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_rejects_whitespace_symlink_alias() {
        let err = parse_invocation(&["bad alias".into()])
            .expect_err("whitespace symlink alias should be rejected");
        assert!(
            err.to_string()
                .contains("alias name cannot contain whitespace"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_rejects_separator_symlink_alias() {
        let err = parse_invocation(&["--".into()])
            .expect_err("separator symlink alias should be rejected");
        assert!(
            err.to_string().contains("alias name cannot be `--`"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_rejects_pathlike_symlink_alias() {
        let err = parse_invocation(&["bad\\alias".into()])
            .expect_err("path-like symlink alias should be rejected");
        assert!(
            err.to_string()
                .contains("alias name cannot contain path separators"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_rejects_windows_relative_pathlike_symlink_alias() {
        let err = parse_invocation(&[".\\badalias".into()])
            .expect_err("windows-relative path-like symlink alias should be rejected");
        assert!(
            err.to_string()
                .contains("alias name cannot contain path separators"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_rejects_parent_windows_relative_pathlike_symlink_alias() {
        let err = parse_invocation(&["..\\badalias".into()])
            .expect_err("parent windows-relative path-like symlink alias should be rejected");
        assert!(
            err.to_string()
                .contains("alias name cannot contain path separators"),
            "{err}"
        );
    }

    #[test]
    fn parse_invocation_with_dot_argv0_uses_direct_mode_error() {
        let err =
            parse_invocation(&[".".into()]).expect_err("dot argv0 should map to direct-mode flow");
        assert!(err.to_string().contains("missing alias name"), "{err}");
    }

    #[test]
    fn parse_invocation_with_parent_argv0_uses_direct_mode_error() {
        let err = parse_invocation(&["..".into()])
            .expect_err("parent argv0 should map to direct-mode flow");
        assert!(err.to_string().contains("missing alias name"), "{err}");
    }

    #[test]
    fn rejects_alias_starting_with_dash() {
        let err = validate_alias_name("-alias").expect_err("dash-prefixed alias is invalid");
        assert!(err.to_string().contains("cannot start with `-`"));
    }

    #[test]
    fn rejects_alias_with_whitespace() {
        let err = validate_alias_name("foo bar").expect_err("whitespace aliases are invalid");
        assert!(err.to_string().contains("cannot contain whitespace"));
    }

    #[test]
    fn find_config_ignores_directory_candidates() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let temp = TempDir::new().expect("create temp config dir");
        let aliases_dir = temp.path().join("aliases");
        fs::create_dir_all(&aliases_dir).expect("create aliases dir");
        fs::create_dir_all(aliases_dir.join("demo.toml")).expect("create directory candidate");
        let root_toml = temp.path().join("demo.toml");
        fs::write(&root_toml, "exec = \"echo\"\n").expect("write fallback config");

        env::set_var("CHOPPER_CONFIG_DIR", temp.path());
        let found = find_config("demo").expect("expected fallback config");
        assert_eq!(found, root_toml);
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
}
