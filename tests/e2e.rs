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
