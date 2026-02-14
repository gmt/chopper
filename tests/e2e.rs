use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

fn chopper_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_chopper"))
}

fn run_chopper(config_home: &TempDir, cache_home: &TempDir, args: &[&str]) -> Output {
    run_chopper_with(
        chopper_bin(),
        config_home,
        cache_home,
        args,
        std::iter::empty::<(&str, String)>(),
    )
}

fn run_chopper_with(
    executable: PathBuf,
    config_home: &TempDir,
    cache_home: &TempDir,
    args: &[&str],
    env_vars: impl IntoIterator<Item = (&'static str, String)>,
) -> Output {
    let mut cmd = Command::new(executable);
    cmd.args(args)
        .env("XDG_CONFIG_HOME", config_home.path())
        .env("XDG_CACHE_HOME", cache_home.path());
    for (key, value) in env_vars {
        cmd.env(key, value);
    }
    cmd.output().expect("failed to run chopper")
}

fn write_executable_script(path: &Path, body: &str) {
    fs::write(path, body).expect("write script");
    let mut perms = fs::metadata(path).expect("script metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("set executable permissions");
}

#[test]
fn help_flag_prints_usage_without_alias() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &["--help"]);
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
    assert!(stdout.contains("CHOPPER_DISABLE_CACHE"), "{stdout}");
}

#[test]
fn short_help_flag_prints_usage_without_alias() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &["-h"]);
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn version_flag_prints_binary_version() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &["--version"]);
    assert!(
        output.status.success(),
        "version command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "expected version in output: {stdout}"
    );
}

#[test]
fn short_version_flag_prints_binary_version() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &["-V"]);
    assert!(
        output.status.success(),
        "short version command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "expected version in output: {stdout}"
    );
}

#[test]
fn builtin_flags_with_extra_args_fall_back_to_alias_validation_error() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &["--help", "extra"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper(&config_home, &cache_home, &["-h", "extra"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper(&config_home, &cache_home, &["--print-cache-dir", "extra"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper(&config_home, &cache_home, &["--version", "extra"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper(&config_home, &cache_home, &["-V", "extra"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper(&config_home, &cache_home, &["--print-config-dir", "extra"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn print_dir_builtins_report_resolved_override_paths() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let override_config = TempDir::new().expect("create override config");
    let override_cache = TempDir::new().expect("create override cache");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        [(
            "CHOPPER_CONFIG_DIR",
            override_config.path().display().to_string(),
        )],
    );
    assert!(
        output.status.success(),
        "print-config-dir failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), override_config.path().display().to_string());

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        [(
            "CHOPPER_CACHE_DIR",
            override_cache.path().display().to_string(),
        )],
    );
    assert!(
        output.status.success(),
        "print-cache-dir failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), override_cache.path().display().to_string());
}

#[test]
fn print_dir_builtins_default_to_xdg_roots() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &["--print-config-dir"]);
    assert!(
        output.status.success(),
        "print-config-dir failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper(&config_home, &cache_home, &["--print-cache-dir"]);
    assert!(
        output.status.success(),
        "print-cache-dir failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_ignore_blank_overrides() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        [("CHOPPER_CONFIG_DIR", "   ".to_string())],
    );
    assert!(
        output.status.success(),
        "print-config-dir failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        [("CHOPPER_CACHE_DIR", "   ".to_string())],
    );
    assert!(
        output.status.success(),
        "print-cache-dir failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn symlink_mode_does_not_treat_help_as_builtin() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("helpcheck.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("helpcheck");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["--help"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=--help"), "{stdout}");
    assert!(!stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn symlink_mode_does_not_treat_print_config_dir_as_builtin() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("printcheck.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("printcheck");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=--print-config-dir"), "{stdout}");
}

#[test]
fn symlink_mode_does_not_treat_print_cache_dir_as_builtin() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("printcachecheck.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("printcachecheck");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=--print-cache-dir"), "{stdout}");
}

#[test]
fn symlink_mode_does_not_treat_version_as_builtin() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("versioncheck.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("versioncheck");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["--version"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=--version"), "{stdout}");
    assert!(!stdout.contains(env!("CARGO_PKG_VERSION")), "{stdout}");
}

#[test]
fn symlink_invocation_uses_symlink_name_as_alias() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("kpods.toml"),
        r#"
exec = "echo"
args = ["symlink-mode"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("kpods");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("symlink-mode runtime"), "{stdout}");
}

#[test]
fn symlink_invocation_preserves_alias_name_with_dots() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("kpods.prod.toml"),
        r#"
exec = "echo"
args = ["symlink-dot-mode"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("kpods.prod");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("symlink-dot-mode runtime"), "{stdout}");
}

#[test]
fn symlink_aliases_that_sanitize_to_same_cache_prefix_do_not_collide() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("alpha:beta.toml"),
        r#"
exec = "echo"
args = ["alias=colon"]
"#,
    )
    .expect("write colon alias config");
    fs::write(
        aliases_dir.join("alpha?beta.toml"),
        r#"
exec = "echo"
args = ["alias=question"]
"#,
    )
    .expect("write question alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let alias_colon = bin_dir.path().join("alpha:beta");
    let alias_question = bin_dir.path().join("alpha?beta");
    symlink(chopper_bin(), &alias_colon).expect("create colon symlink");
    symlink(chopper_bin(), &alias_question).expect("create question symlink");

    let output = run_chopper_with(
        alias_colon,
        &config_home,
        &cache_home,
        &["runtime-a"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "colon command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("alias=colon runtime-a"), "{stdout}");

    let output = run_chopper_with(
        alias_question,
        &config_home,
        &cache_home,
        &["runtime-b"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "question command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("alias=question runtime-b"), "{stdout}");

    let manifests_dir = cache_home.path().join("chopper/manifests");
    let matching_cache_entries = fs::read_dir(&manifests_dir)
        .expect("read manifests dir")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("alpha_beta-") && name.ends_with(".bin"))
        .count();
    assert_eq!(
        matching_cache_entries, 2,
        "expected one cache entry per colliding-sanitization alias in {:?}",
        manifests_dir
    );
}

#[test]
fn symlink_invocation_strips_double_dash_separator() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("kpods.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("kpods");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["--", "--tail=100"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=base --tail=100"), "{stdout}");
    assert!(!stdout.contains("ARGS=base -- --tail=100"), "{stdout}");
}

#[test]
fn missing_alias_config_falls_back_to_path_command_resolution() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let fake_bin = TempDir::new().expect("create fake-bin dir");
    let command_path = fake_bin.path().join("fallbackcmd");
    write_executable_script(
        &command_path,
        "#!/usr/bin/env bash\nprintf 'PATH_FALLBACK=%s\\n' \"$*\"\n",
    );

    let existing_path = std::env::var("PATH").unwrap_or_default();
    let merged_path = format!("{}:{existing_path}", fake_bin.path().display());
    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["fallbackcmd", "runtime"],
        [("PATH", merged_path)],
    );

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PATH_FALLBACK=runtime"), "{stdout}");
}

#[test]
fn missing_alias_config_reports_clear_exec_failure_when_command_missing() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["definitely-missing-command-xyz"],
        [("PATH", "/nonexistent".to_string())],
    );

    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("exec failed"), "{stderr}");
    assert!(stderr.contains("No such file or directory"), "{stderr}");
}

#[test]
fn alias_lookup_order_prefers_aliases_toml_then_root_toml_then_legacy() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    let aliases_dir = chopper_dir.join("aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    let aliases_toml = aliases_dir.join("lookup.toml");
    let root_toml = chopper_dir.join("lookup.toml");
    let legacy = chopper_dir.join("lookup");

    fs::write(
        &aliases_toml,
        r#"
exec = "echo"
args = ["source=aliases"]
"#,
    )
    .expect("write aliases toml");
    fs::write(
        &root_toml,
        r#"
exec = "echo"
args = ["source=root-toml"]
"#,
    )
    .expect("write root toml");
    fs::write(&legacy, "echo source=legacy").expect("write legacy alias");

    let output = run_chopper(&config_home, &cache_home, &["lookup"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("source=aliases"), "{stdout}");

    fs::remove_file(&aliases_toml).expect("remove aliases toml");
    let output = run_chopper(&config_home, &cache_home, &["lookup"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("source=root-toml"), "{stdout}");

    fs::remove_file(&root_toml).expect("remove root toml");
    let output = run_chopper(&config_home, &cache_home, &["lookup"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("source=legacy"), "{stdout}");
}

#[test]
fn journal_config_forwards_stderr_to_systemd_cat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("journaled.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'OUT_STREAM\n'; printf 'ERR_STREAM\n' 1>&2"]

[journal]
namespace = "ops-e2e"
stderr = true
identifier = "journal-test"
"#,
    )
    .expect("write alias config");

    let fake_bin = TempDir::new().expect("create fake-bin dir");
    let captured_err = fake_bin.path().join("captured-stderr.log");
    let captured_args = fake_bin.path().join("captured-args.log");
    let script_path = fake_bin.path().join("systemd-cat");
    write_executable_script(
        &script_path,
        &format!(
            "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > \"{}\"\ncat > \"{}\"\n",
            captured_args.display(),
            captured_err.display()
        ),
    );

    let existing_path = std::env::var("PATH").unwrap_or_default();
    let merged_path = format!("{}:{existing_path}", fake_bin.path().display());
    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["journaled"],
        [("PATH", merged_path)],
    );

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OUT_STREAM"), "{stdout}");
    assert!(
        !stdout.contains("ERR_STREAM"),
        "stderr should be redirected to systemd-cat: {stdout}"
    );

    let captured_err_text =
        fs::read_to_string(&captured_err).expect("read captured systemd-cat stdin");
    assert!(
        captured_err_text.contains("ERR_STREAM"),
        "captured stderr text: {captured_err_text}"
    );
    let captured_args_text =
        fs::read_to_string(&captured_args).expect("read captured systemd-cat args");
    assert!(
        captured_args_text.contains("--namespace=ops-e2e"),
        "captured args: {captured_args_text}"
    );
    assert!(
        captured_args_text.contains("--identifier=journal-test"),
        "captured args: {captured_args_text}"
    );
}

#[test]
fn journal_parser_trimming_uses_trimmed_namespace_and_drops_blank_identifier() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("journal-trimmed.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ERR_STREAM\n' 1>&2"]

[journal]
namespace = "  ops-e2e  "
stderr = true
identifier = "   "
"#,
    )
    .expect("write alias config");

    let fake_bin = TempDir::new().expect("create fake-bin dir");
    let captured_err = fake_bin.path().join("captured-stderr.log");
    let captured_args = fake_bin.path().join("captured-args.log");
    let script_path = fake_bin.path().join("systemd-cat");
    write_executable_script(
        &script_path,
        &format!(
            "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > \"{}\"\ncat > \"{}\"\n",
            captured_args.display(),
            captured_err.display()
        ),
    );

    let existing_path = std::env::var("PATH").unwrap_or_default();
    let merged_path = format!("{}:{existing_path}", fake_bin.path().display());
    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["journal-trimmed"],
        [("PATH", merged_path)],
    );

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let captured_err_text =
        fs::read_to_string(&captured_err).expect("read captured systemd-cat stdin");
    assert!(
        captured_err_text.contains("ERR_STREAM"),
        "{captured_err_text}"
    );

    let captured_args_text =
        fs::read_to_string(&captured_args).expect("read captured systemd-cat args");
    assert!(
        captured_args_text.contains("--namespace=ops-e2e"),
        "captured args: {captured_args_text}"
    );
    assert!(
        !captured_args_text.contains("--identifier="),
        "blank identifier should be omitted: {captured_args_text}"
    );
}

#[test]
fn direct_invocation_with_alias_name_still_works() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("demo.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"; printf 'ENV=%s\n' \"$CHOPPER_E2E\"", "_", "base"]

[env]
CHOPPER_E2E = "from_alias"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["demo", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=base runtime"), "{stdout}");
    assert!(stdout.contains("ENV=from_alias"), "{stdout}");

    let cache_entry = cache_home.path().join("chopper/manifests/demo.bin");
    assert!(
        cache_entry.exists(),
        "expected cache entry at {:?}",
        cache_entry
    );
}

#[test]
fn parser_trimming_is_applied_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("trimmed.reconcile.rhai"),
        r#"
fn trimmed_reconcile(_ctx) {
  #{
    append_args: ["from_trimmed_reconcile"]
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("trimmed.toml"),
        r#"
exec = "  sh  "
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]

[reconcile]
script = "  trimmed.reconcile.rhai  "
function = "  trimmed_reconcile  "
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["trimmed", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ARGS=base runtime from_trimmed_reconcile"),
        "{stdout}"
    );
}

#[test]
fn direct_invocation_strips_double_dash_separator() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dash.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dash", "--", "--tail=100"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=base --tail=100"), "{stdout}");
    assert!(!stdout.contains("ARGS=base -- --tail=100"), "{stdout}");
}

#[test]
fn direct_invocation_rejects_separator_as_alias_name() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &["--", "runtime"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("alias name cannot be `--`"), "{stderr}");
}

#[test]
fn direct_invocation_rejects_pathlike_alias_name() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &["foo/bar", "runtime"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("path separators"), "{stderr}");
}

#[test]
fn direct_invocation_rejects_dot_alias_tokens() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &[".", "runtime"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot be `.` or `..`"), "{stderr}");

    let output = run_chopper(&config_home, &cache_home, &["..", "runtime"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot be `.` or `..`"), "{stderr}");
}

#[test]
fn direct_invocation_rejects_dash_prefixed_alias_tokens() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &["--version", "runtime"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn direct_invocation_rejects_whitespace_alias_tokens() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &["foo bar", "runtime"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot contain whitespace"), "{stderr}");
}

#[test]
fn explicit_config_and_cache_override_env_vars_are_honored() {
    let config_home = TempDir::new().expect("create xdg config home");
    let cache_home = TempDir::new().expect("create xdg cache home");
    let override_config_root = TempDir::new().expect("create override config root");
    let override_cache_root = TempDir::new().expect("create override cache root");

    let aliases_dir = override_config_root.path().join("aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::write(
        aliases_dir.join("override.toml"),
        r#"
exec = "echo"
args = ["override-root"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["override", "runtime"],
        [
            (
                "CHOPPER_CONFIG_DIR",
                override_config_root.path().display().to_string(),
            ),
            (
                "CHOPPER_CACHE_DIR",
                override_cache_root.path().display().to_string(),
            ),
        ],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("override-root runtime"), "{stdout}");

    let override_cache_file = override_cache_root
        .path()
        .join("manifests")
        .join("override.bin");
    assert!(
        override_cache_file.exists(),
        "expected cache at override path: {:?}",
        override_cache_file
    );

    let default_cache_file = cache_home.path().join("chopper/manifests/override.bin");
    assert!(
        !default_cache_file.exists(),
        "cache should not be written into default XDG cache when CHOPPER_CACHE_DIR is set: {:?}",
        default_cache_file
    );
}

#[test]
fn empty_config_and_cache_overrides_fall_back_to_xdg_roots() {
    let config_home = TempDir::new().expect("create xdg config home");
    let cache_home = TempDir::new().expect("create xdg cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("fallback.toml"),
        r#"
exec = "echo"
args = ["fallback-root"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["fallback", "runtime"],
        [
            ("CHOPPER_CONFIG_DIR", "   ".to_string()),
            ("CHOPPER_CACHE_DIR", "   ".to_string()),
        ],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fallback-root runtime"), "{stdout}");

    let default_cache_file = cache_home.path().join("chopper/manifests/fallback.bin");
    assert!(
        default_cache_file.exists(),
        "expected cache file in default XDG cache root: {:?}",
        default_cache_file
    );
}

#[test]
fn cache_can_be_disabled_for_extraordinary_debugging() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("nocache.toml"),
        r#"
exec = "echo"
args = ["cache-bypass"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["nocache", "runtime"],
        [("CHOPPER_DISABLE_CACHE", "1".to_string())],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cache-bypass runtime"), "{stdout}");

    let cache_file = cache_home.path().join("chopper/manifests/nocache.bin");
    assert!(
        !cache_file.exists(),
        "cache file should not be written when disabled: {:?}",
        cache_file
    );
}

#[test]
fn cache_disable_flag_is_case_insensitive_in_e2e_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("nocache-case.toml"),
        r#"
exec = "echo"
args = ["cache-bypass-case"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["nocache-case", "runtime"],
        [("CHOPPER_DISABLE_CACHE", "TrUe".to_string())],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cache-bypass-case runtime"), "{stdout}");

    let cache_file = cache_home.path().join("chopper/manifests/nocache-case.bin");
    assert!(
        !cache_file.exists(),
        "cache file should not be written when disabled with mixed-case value: {:?}",
        cache_file
    );
}

#[test]
fn cache_disable_flag_falsey_value_keeps_cache_enabled() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("nocache-falsey.toml"),
        r#"
exec = "echo"
args = ["cache-enabled"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["nocache-falsey"],
        [("CHOPPER_DISABLE_CACHE", "0".to_string())],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cache_file = cache_home
        .path()
        .join("chopper/manifests/nocache-falsey.bin");
    assert!(
        cache_file.exists(),
        "cache file should be written when disable flag is falsey: {:?}",
        cache_file
    );
}

#[test]
fn cache_invalidation_applies_updated_alias_config() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    let alias_path = aliases_dir.join("mutable.toml");

    fs::write(
        &alias_path,
        r#"
exec = "echo"
args = ["before-change"]
"#,
    )
    .expect("write alias config");

    let first = run_chopper(&config_home, &cache_home, &["mutable"]);
    assert!(
        first.status.success(),
        "first run failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    assert!(first_stdout.contains("before-change"), "{first_stdout}");

    fs::write(
        &alias_path,
        r#"
exec = "echo"
args = ["after-change"]
"#,
    )
    .expect("rewrite alias config");

    let second = run_chopper(&config_home, &cache_home, &["mutable"]);
    assert!(
        second.status.success(),
        "second run failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_stdout = String::from_utf8_lossy(&second.stdout);
    assert!(second_stdout.contains("after-change"), "{second_stdout}");
    assert!(!second_stdout.contains("before-change"), "{second_stdout}");
}

#[test]
fn reconcile_script_can_append_args_and_override_env() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("hook.reconcile.rhai"),
        r#"
fn reconcile(ctx) {
  let out = #{};
  if ctx.runtime_args.contains("--loud") {
    out["append_args"] = ["from_reconcile"];
    out["set_env"] = #{ "CHOPPER_E2E": "from_reconcile" };
  }
  out
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("hooked.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"; printf 'ENV=%s\n' \"$CHOPPER_E2E\"", "_", "base"]

[env]
CHOPPER_E2E = "from_alias"

[reconcile]
script = "hook.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["hooked", "--loud", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ARGS=base --loud runtime from_reconcile"),
        "{stdout}"
    );
    assert!(stdout.contains("ENV=from_reconcile"), "{stdout}");
}

#[test]
fn reconcile_can_read_runtime_environment_from_context() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("runtime-env.reconcile.rhai"),
        r#"
fn reconcile(ctx) {
  let out = #{};
  out["set_env"] = #{ "CHOPPER_E2E": ctx.runtime_env["RUNTIME_MARKER"] };
  out
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("runtime-env.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ENV=%s\n' \"$CHOPPER_E2E\""]

[env]
CHOPPER_E2E = "from_alias"

[reconcile]
script = "runtime-env.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["runtime-env"],
        [("RUNTIME_MARKER", "from_runtime_env".to_string())],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ENV=from_runtime_env"), "{stdout}");
}

#[test]
fn reconcile_can_be_disabled_for_extraordinary_debugging() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("toggle.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    append_args: ["from_reconcile"],
    set_env: #{ "CHOPPER_E2E": "from_reconcile" }
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("toggle.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"; printf 'ENV=%s\n' \"$CHOPPER_E2E\"", "_", "base"]

[env]
CHOPPER_E2E = "from_alias"

[reconcile]
script = "toggle.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["toggle", "runtime"],
        [("CHOPPER_DISABLE_RECONCILE", "1".to_string())],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=base runtime"), "{stdout}");
    assert!(!stdout.contains("from_reconcile"), "{stdout}");
    assert!(stdout.contains("ENV=from_alias"), "{stdout}");
}

#[test]
fn reconcile_disable_flag_is_case_insensitive_in_e2e_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("toggle-case.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    append_args: ["from_reconcile_case"]
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("toggle-case.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]

[reconcile]
script = "toggle-case.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["toggle-case", "runtime"],
        [("CHOPPER_DISABLE_RECONCILE", "YeS".to_string())],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=base runtime"), "{stdout}");
    assert!(!stdout.contains("from_reconcile_case"), "{stdout}");
}

#[test]
fn reconcile_disable_flag_falsey_value_keeps_reconcile_enabled() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("toggle-falsey.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    append_args: ["from_reconcile_falsey"]
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("toggle-falsey.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]

[reconcile]
script = "toggle-falsey.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["toggle-falsey", "runtime"],
        [("CHOPPER_DISABLE_RECONCILE", "0".to_string())],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ARGS=base runtime from_reconcile_falsey"),
        "{stdout}"
    );
}

#[test]
fn journal_config_surfaces_systemd_cat_failure_with_hint() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("journal-fail.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'OUT_STREAM\n'; printf 'ERR_STREAM\n' 1>&2"]

[journal]
namespace = "ops-e2e"
stderr = true
"#,
    )
    .expect("write alias config");

    let fake_bin = TempDir::new().expect("create fake-bin dir");
    let script_path = fake_bin.path().join("systemd-cat");
    write_executable_script(
        &script_path,
        "#!/usr/bin/env bash\ncat >/dev/null\nexit 17\n",
    );

    let existing_path = std::env::var("PATH").unwrap_or_default();
    let merged_path = format!("{}:{existing_path}", fake_bin.path().display());
    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["journal-fail"],
        [("PATH", merged_path)],
    );

    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OUT_STREAM"), "{stdout}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("journal namespace requires systemd-cat --namespace support"),
        "{stderr}"
    );
}

#[test]
fn journal_failure_before_child_spawn_avoids_side_effects() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    let side_effect = config_home.path().join("should-not-exist");

    fs::write(
        aliases_dir.join("journal-no-child.toml"),
        format!(
            r#"
exec = "/bin/sh"
args = ["-c", "touch \"$1\"", "_", "{}"]

[journal]
namespace = "ops-e2e"
stderr = true
"#,
            side_effect.display()
        ),
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["journal-no-child"],
        [("PATH", "/nonexistent".to_string())],
    );

    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to spawn systemd-cat"), "{stderr}");
    assert!(
        !side_effect.exists(),
        "child command should not run when journal spawn fails"
    );
}

#[test]
fn journal_stderr_false_skips_systemd_cat_dependency() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("journal-no-stderr.toml"),
        r#"
exec = "/bin/sh"
args = ["-c", "printf 'OUT_STREAM\n'; printf 'ERR_STREAM\n' 1>&2"]

[journal]
namespace = "ops-e2e"
stderr = false
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["journal-no-stderr"],
        [("PATH", "/nonexistent".to_string())],
    );

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("OUT_STREAM"), "{stdout}");
    assert!(stderr.contains("ERR_STREAM"), "{stderr}");
}

#[test]
fn reconcile_can_replace_args_and_remove_env() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("replace.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    replace_args: [
      "-c",
      "printf 'ARGS=%s\n' \"$*\"; printf 'DROP=%s\n' \"$CHOPPER_DROP\"; printf 'KEEP=%s\n' \"$CHOPPER_KEEP\"",
      "_",
      "replaced"
    ],
    remove_env: ["CHOPPER_DROP"],
    set_env: #{ "CHOPPER_KEEP": "overridden" }
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("replace.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]

[env]
CHOPPER_DROP = "drop-me"
CHOPPER_KEEP = "from-alias"

[reconcile]
script = "replace.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["replace", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=replaced"), "{stdout}");
    assert!(!stdout.contains("ARGS=base runtime"), "{stdout}");
    assert!(stdout.contains("DROP="), "{stdout}");
    assert!(stdout.contains("KEEP=overridden"), "{stdout}");
}

#[test]
fn reconcile_set_env_overrides_alias_env_remove() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("promote.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    set_env: #{ "PROMOTE": "from_reconcile" }
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("promote.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'PROMOTE=%s\n' \"$PROMOTE\""]
env_remove = ["PROMOTE"]

[reconcile]
script = "promote.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["promote"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PROMOTE=from_reconcile"), "{stdout}");
}

#[test]
fn static_env_remove_unsets_inherited_environment_values() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("envremove.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'DROP=%s\n' \"$CHOPPER_DROP\"; printf 'KEEP=%s\n' \"$CHOPPER_KEEP\""]
env_remove = ["CHOPPER_DROP"]

[env]
CHOPPER_KEEP = "from_alias"
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["envremove"],
        [
            ("CHOPPER_DROP", "from_runtime".to_string()),
            ("CHOPPER_KEEP", "from_runtime".to_string()),
        ],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("DROP="), "{stdout}");
    assert!(!stdout.contains("DROP=from_runtime"), "{stdout}");
    assert!(stdout.contains("KEEP=from_alias"), "{stdout}");
}

#[test]
fn env_remove_trimming_applies_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("envremove-trimmed.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'DROP=%s\n' \"$CHOPPER_DROP\"; printf 'KEEP=%s\n' \"$CHOPPER_KEEP\""]
env_remove = ["  CHOPPER_DROP  ", "   "]

[env]
CHOPPER_KEEP = "from_alias"
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["envremove-trimmed"],
        [
            ("CHOPPER_DROP", "from_runtime".to_string()),
            ("CHOPPER_KEEP", "from_runtime".to_string()),
        ],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("DROP="), "{stdout}");
    assert!(!stdout.contains("DROP=from_runtime"), "{stdout}");
    assert!(stdout.contains("KEEP=from_alias"), "{stdout}");
}

#[test]
fn reconcile_function_name_override_is_honored() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("customfn.reconcile.rhai"),
        r#"
fn custom_reconcile(_ctx) {
  #{
    append_args: ["from_custom_function"]
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("customfn.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]

[reconcile]
script = "customfn.reconcile.rhai"
function = "custom_reconcile"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["customfn", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ARGS=base runtime from_custom_function"),
        "{stdout}"
    );
}

#[test]
fn legacy_one_line_alias_remains_supported() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(chopper_dir.join("legacy"), "echo legacy").expect("write legacy alias");

    let output = run_chopper(&config_home, &cache_home, &["legacy", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("legacy runtime"), "{stdout}");
}

#[test]
fn toml_env_duplicate_keys_after_trim_fail_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dup-env.toml"),
        r#"
exec = "echo"

[env]
FOO = "base"
" FOO " = "collision"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dup-env"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("contains duplicate keys after trimming"),
        "{stderr}"
    );
}
