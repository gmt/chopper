use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;

fn chopper_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_chopper"))
}

fn run_chopper(config_home: &TempDir, cache_home: &TempDir, args: &[&str]) -> Output {
    Command::new(chopper_bin())
        .args(args)
        .env("XDG_CONFIG_HOME", config_home.path())
        .env("XDG_CACHE_HOME", cache_home.path())
        .output()
        .expect("failed to run chopper")
}

#[test]
fn toml_alias_supports_args_env_and_cache() {
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
