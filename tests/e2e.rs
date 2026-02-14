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
