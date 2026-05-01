use anyhow::{anyhow, Result};
use std::path::PathBuf;

pub(crate) fn run(args: &[String]) -> Result<()> {
    let invocation = parse_invocation(args)?;

    let config_path = find_config(&invocation.alias);
    let manifest = match config_path {
        Some(path) => load_manifest(&invocation.alias, &path)?,
        None => crate::manifest::Manifest::simple(crate::exec_resolution::resolve_command_path(
            &invocation.alias,
        )),
    };

    let patch = crate::reconcile::maybe_reconcile(&manifest, &invocation.passthrough_args)?;
    let resolved = manifest.build_invocation(&invocation.passthrough_args, patch)?;
    crate::executor::run(resolved)
}

fn config_dir() -> PathBuf {
    if let Some(override_path) = crate::env_util::env_path_override("CHOPPER_CONFIG_DIR") {
        return override_path;
    }

    directories::ProjectDirs::from("", "", "chopper")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".chopper"))
}

fn find_config(name: &str) -> Option<PathBuf> {
    crate::alias_paths::find_exec_config(&config_dir(), name)
}

fn load_manifest(alias: &str, path: &std::path::Path) -> Result<crate::manifest::Manifest> {
    if !cache_enabled() {
        return crate::parser::parse(path);
    }

    let fingerprint = crate::cache::source_fingerprint(path)?;
    if let Some(cached) = crate::cache::load(alias, &fingerprint) {
        return Ok(cached);
    }

    let manifest = crate::parser::parse(path)?;
    crate::cache::store(alias, &fingerprint, &manifest)?;
    Ok(manifest)
}

fn cache_enabled() -> bool {
    !crate::env_util::env_flag_enabled("CHOPPER_DISABLE_CACHE")
}

#[derive(Debug, PartialEq, Eq)]
struct InvocationInput {
    alias: String,
    passthrough_args: Vec<String>,
}

fn parse_invocation(args: &[String]) -> Result<InvocationInput> {
    let exe_name = invocation_executable_name(args);

    if is_direct_invocation_executable(&exe_name) {
        if args.len() < 2 {
            return Err(anyhow!(
                "missing alias name; use `chopper-exe <alias> [args...]`"
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
        Err(AliasViolation::ContainsNul) => Err(anyhow!("alias name cannot contain NUL bytes")),
        Err(AliasViolation::IsSeparator) => Err(anyhow!(
            "alias name cannot be `--`; expected `chopper-exe <alias> -- [args...]`"
        )),
        Err(AliasViolation::StartsWithDash) => Err(anyhow!(
            "alias name cannot start with `-`; choose a non-flag alias name"
        )),
        Err(AliasViolation::ContainsWhitespace) => Err(anyhow!("alias name cannot contain whitespace")),
        Err(AliasViolation::IsDotToken) => Err(anyhow!("alias name cannot be `.` or `..`")),
        Err(AliasViolation::ContainsPathSeparator) => Err(anyhow!(
            "alias name cannot contain path separators; use symlink mode or command PATH resolution instead"
        )),
    }
}

fn invocation_executable_name(args: &[String]) -> String {
    let raw = args.first().map(String::as_str).unwrap_or("chopper-exe");
    let basename = if raw.contains('/') {
        raw.trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("chopper-exe")
    } else {
        raw
    };
    let basename = if basename.is_empty() || basename == "." || basename == ".." {
        "chopper-exe"
    } else {
        basename
    };

    PathBuf::from(basename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("chopper-exe")
        .to_string()
}

fn is_direct_invocation_executable(exe_name: &str) -> bool {
    exe_name.eq_ignore_ascii_case("chopper-exe") || exe_name.eq_ignore_ascii_case("chopper_exe")
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
    use super::parse_invocation;

    #[test]
    fn supports_direct_invocation_mode() {
        let invocation = parse_invocation(&[
            "chopper-exe".to_string(),
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
}
