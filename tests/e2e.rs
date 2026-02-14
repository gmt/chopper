use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::process::CommandExt;
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

fn run_chopper_with_cwd_and_argv0(
    executable: PathBuf,
    argv0: &str,
    working_dir: &Path,
    config_home: &TempDir,
    cache_home: &TempDir,
    args: &[&str],
    env_vars: impl IntoIterator<Item = (&'static str, String)>,
) -> Output {
    let mut cmd = Command::new(executable);
    cmd.arg0(argv0)
        .current_dir(working_dir)
        .args(args)
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
fn no_args_prints_usage_without_alias() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let output = run_chopper(&config_home, &cache_home, &[]);
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
    assert!(stdout.contains("Built-ins:"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("chopper.exe");
    symlink(chopper_bin(), &chopper_exe).expect("create chopper.exe symlink");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_uppercase_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("CHOPPER.EXE");
    symlink(chopper_bin(), &chopper_exe).expect("create CHOPPER.EXE symlink");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("chopper.cmd");
    symlink(chopper_bin(), &chopper_cmd).expect("create chopper.cmd symlink");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("chopper.bat");
    symlink(chopper_bin(), &chopper_bat).expect("create chopper.bat symlink");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("chopper.com");
    symlink(chopper_bin(), &chopper_com).expect("create chopper.com symlink");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("CHOPPER.COM");
    symlink(chopper_bin(), &chopper_com).expect("create CHOPPER.COM symlink");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("CHOPPER.CMD");
    symlink(chopper_bin(), &chopper_cmd).expect("create CHOPPER.CMD symlink");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_uppercase_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("CHOPPER.BAT");
    symlink(chopper_bin(), &chopper_bat).expect("create CHOPPER.BAT symlink");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_uppercase_chopper() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let uppercase_chopper = bin_dir.path().join("CHOPPER");
    symlink(chopper_bin(), &uppercase_chopper).expect("create CHOPPER symlink");

    let output = run_chopper_with(
        uppercase_chopper,
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_windows_relative_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = ".\\CHOPPER.CMD";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn windows_relative_uppercase_chopper_cmd_supports_direct_alias_invocation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::write(
        aliases_dir.join("winrel.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = ".\\CHOPPER.CMD";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["winrel", "runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "direct invocation via windows-relative CHOPPER.CMD failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=runtime"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_parent_windows_relative_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "..\\CHOPPER.COM";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn parent_windows_relative_uppercase_chopper_com_supports_direct_alias_invocation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::write(
        aliases_dir.join("winrelparent.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "..\\CHOPPER.COM";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["winrelparent", "runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "direct invocation via parent windows-relative CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=runtime"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_unc_windows_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "\\\\server\\tools\\CHOPPER.CMD";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn unc_windows_uppercase_chopper_com_supports_direct_alias_invocation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::write(
        aliases_dir.join("uncwin.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "\\\\server\\tools\\CHOPPER.COM";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["uncwin", "runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "direct invocation via UNC windows CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=runtime"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_drive_windows_uppercase_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "D:\\bin\\CHOPPER.BAT";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn drive_windows_uppercase_chopper_cmd_supports_direct_alias_invocation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::write(
        aliases_dir.join("drivewin.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "C:\\tools\\CHOPPER.CMD";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["drivewin", "runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "direct invocation via drive windows CHOPPER.CMD failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=runtime"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_drive_windows_forward_slash_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "C:/tools/CHOPPER.CMD";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn forward_slash_unc_uppercase_chopper_com_supports_direct_alias_invocation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::write(
        aliases_dir.join("uncfwd.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "//server/tools/CHOPPER.COM";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["uncfwd", "runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "direct invocation via forward-slash UNC CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=runtime"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_mixed_separator_drive_windows_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "C:/tools\\CHOPPER.CMD";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn mixed_separator_unc_uppercase_chopper_bat_supports_direct_alias_invocation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::write(
        aliases_dir.join("uncmix.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "\\\\server/tools\\CHOPPER.BAT";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["uncmix", "runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "direct invocation via mixed-separator UNC CHOPPER.BAT failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=runtime"), "{stdout}");
}

#[test]
fn no_args_prints_usage_when_invoked_as_unix_relative_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "./CHOPPER.COM";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "no-args command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn unix_parent_relative_uppercase_chopper_cmd_supports_direct_alias_invocation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::write(
        aliases_dir.join("unixrel.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "../CHOPPER.CMD";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["unixrel", "runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "direct invocation via unix parent-relative CHOPPER.CMD failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=runtime"), "{stdout}");
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
fn short_help_flag_prints_usage_when_invoked_as_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("chopper.exe");
    symlink(chopper_bin(), &chopper_exe).expect("create chopper.exe symlink");

    let output = run_chopper_with(
        chopper_exe,
        &config_home,
        &cache_home,
        &["-h"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn short_help_flag_prints_usage_when_invoked_as_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("chopper.cmd");
    symlink(chopper_bin(), &chopper_cmd).expect("create chopper.cmd symlink");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["-h"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn short_help_flag_prints_usage_when_invoked_as_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("chopper.bat");
    symlink(chopper_bin(), &chopper_bat).expect("create chopper.bat symlink");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["-h"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn short_help_flag_prints_usage_when_invoked_as_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("chopper.com");
    symlink(chopper_bin(), &chopper_com).expect("create chopper.com symlink");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["-h"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn help_flag_prints_usage_when_invoked_as_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("chopper.cmd");
    symlink(chopper_bin(), &chopper_cmd).expect("create chopper.cmd symlink");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["--help"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn help_flag_prints_usage_when_invoked_as_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("chopper.bat");
    symlink(chopper_bin(), &chopper_bat).expect("create chopper.bat symlink");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["--help"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn help_flag_prints_usage_when_invoked_as_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("chopper.com");
    symlink(chopper_bin(), &chopper_com).expect("create chopper.com symlink");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["--help"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn short_help_flag_prints_usage_when_invoked_as_uppercase_chopper() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let uppercase_chopper = bin_dir.path().join("CHOPPER");
    symlink(chopper_bin(), &uppercase_chopper).expect("create CHOPPER symlink");

    let output = run_chopper_with(
        uppercase_chopper.clone(),
        &config_home,
        &cache_home,
        &["-h"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn short_help_flag_prints_usage_when_invoked_as_uppercase_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("CHOPPER.EXE");
    symlink(chopper_bin(), &chopper_exe).expect("create CHOPPER.EXE symlink");

    let output = run_chopper_with(
        chopper_exe,
        &config_home,
        &cache_home,
        &["-h"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn short_help_flag_prints_usage_when_invoked_as_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("CHOPPER.CMD");
    symlink(chopper_bin(), &chopper_cmd).expect("create CHOPPER.CMD symlink");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["-h"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn short_help_flag_prints_usage_when_invoked_as_uppercase_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("CHOPPER.BAT");
    symlink(chopper_bin(), &chopper_bat).expect("create CHOPPER.BAT symlink");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["-h"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn short_help_flag_prints_usage_when_invoked_as_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("CHOPPER.COM");
    symlink(chopper_bin(), &chopper_com).expect("create CHOPPER.COM symlink");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["-h"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn help_flag_prints_usage_when_invoked_as_uppercase_chopper() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let uppercase_chopper = bin_dir.path().join("CHOPPER");
    symlink(chopper_bin(), &uppercase_chopper).expect("create CHOPPER symlink");

    let output = run_chopper_with(
        uppercase_chopper.clone(),
        &config_home,
        &cache_home,
        &["--help"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn help_flag_prints_usage_when_invoked_as_uppercase_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("CHOPPER.EXE");
    symlink(chopper_bin(), &chopper_exe).expect("create CHOPPER.EXE symlink");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["--help"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn help_flag_prints_usage_when_invoked_as_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("CHOPPER.CMD");
    symlink(chopper_bin(), &chopper_cmd).expect("create CHOPPER.CMD symlink");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["--help"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn help_flag_prints_usage_when_invoked_as_uppercase_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("CHOPPER.BAT");
    symlink(chopper_bin(), &chopper_bat).expect("create CHOPPER.BAT symlink");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["--help"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn help_flag_prints_usage_when_invoked_as_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("CHOPPER.COM");
    symlink(chopper_bin(), &chopper_com).expect("create CHOPPER.COM symlink");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["--help"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn direct_alias_invocation_works_when_invoked_as_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::write(
        aliases_dir.join("fromexe.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("chopper.exe");
    symlink(chopper_bin(), &chopper_exe).expect("create chopper.exe symlink");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["fromexe", "runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "direct invocation via chopper.exe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=runtime"), "{stdout}");
}

#[test]
fn direct_alias_invocation_works_when_invoked_as_uppercase_chopper() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::write(
        aliases_dir.join("fromupper.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let uppercase_chopper = bin_dir.path().join("CHOPPER");
    symlink(chopper_bin(), &uppercase_chopper).expect("create CHOPPER symlink");

    let output = run_chopper_with(
        uppercase_chopper.clone(),
        &config_home,
        &cache_home,
        &["fromupper", "runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "direct invocation via CHOPPER failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=runtime"), "{stdout}");
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
fn short_version_flag_prints_binary_version_when_invoked_as_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("chopper.exe");
    symlink(chopper_bin(), &chopper_exe).expect("create chopper.exe symlink");

    let output = run_chopper_with(
        chopper_exe,
        &config_home,
        &cache_home,
        &["-V"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn short_version_flag_prints_binary_version_when_invoked_as_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("chopper.cmd");
    symlink(chopper_bin(), &chopper_cmd).expect("create chopper.cmd symlink");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["-V"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn short_version_flag_prints_binary_version_when_invoked_as_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("chopper.bat");
    symlink(chopper_bin(), &chopper_bat).expect("create chopper.bat symlink");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["-V"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn short_version_flag_prints_binary_version_when_invoked_as_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("chopper.com");
    symlink(chopper_bin(), &chopper_com).expect("create chopper.com symlink");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["-V"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn version_flag_prints_binary_version_when_invoked_as_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("chopper.cmd");
    symlink(chopper_bin(), &chopper_cmd).expect("create chopper.cmd symlink");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["--version"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn version_flag_prints_binary_version_when_invoked_as_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("chopper.bat");
    symlink(chopper_bin(), &chopper_bat).expect("create chopper.bat symlink");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["--version"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn version_flag_prints_binary_version_when_invoked_as_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("chopper.com");
    symlink(chopper_bin(), &chopper_com).expect("create chopper.com symlink");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["--version"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn short_version_flag_prints_binary_version_when_invoked_as_uppercase_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("CHOPPER.EXE");
    symlink(chopper_bin(), &chopper_exe).expect("create CHOPPER.EXE symlink");

    let output = run_chopper_with(
        chopper_exe,
        &config_home,
        &cache_home,
        &["-V"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn short_version_flag_prints_binary_version_when_invoked_as_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("CHOPPER.CMD");
    symlink(chopper_bin(), &chopper_cmd).expect("create CHOPPER.CMD symlink");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["-V"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn short_version_flag_prints_binary_version_when_invoked_as_uppercase_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("CHOPPER.BAT");
    symlink(chopper_bin(), &chopper_bat).expect("create CHOPPER.BAT symlink");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["-V"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn short_version_flag_prints_binary_version_when_invoked_as_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("CHOPPER.COM");
    symlink(chopper_bin(), &chopper_com).expect("create CHOPPER.COM symlink");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["-V"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn short_version_flag_prints_binary_version_when_invoked_as_uppercase_chopper() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let uppercase_chopper = bin_dir.path().join("CHOPPER");
    symlink(chopper_bin(), &uppercase_chopper).expect("create CHOPPER symlink");

    let output = run_chopper_with(
        uppercase_chopper,
        &config_home,
        &cache_home,
        &["-V"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn version_flag_prints_binary_version_when_invoked_as_uppercase_chopper() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let uppercase_chopper = bin_dir.path().join("CHOPPER");
    symlink(chopper_bin(), &uppercase_chopper).expect("create CHOPPER symlink");

    let output = run_chopper_with(
        uppercase_chopper.clone(),
        &config_home,
        &cache_home,
        &["--version"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn version_flag_prints_binary_version_when_invoked_as_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("CHOPPER.CMD");
    symlink(chopper_bin(), &chopper_cmd).expect("create CHOPPER.CMD symlink");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["--version"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn version_flag_prints_binary_version_when_invoked_as_uppercase_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("CHOPPER.BAT");
    symlink(chopper_bin(), &chopper_bat).expect("create CHOPPER.BAT symlink");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["--version"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn version_flag_prints_binary_version_when_invoked_as_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("CHOPPER.COM");
    symlink(chopper_bin(), &chopper_com).expect("create CHOPPER.COM symlink");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["--version"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn builtin_flags_with_extra_args_via_chopper_exe_fall_back_to_alias_validation_error() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("chopper.exe");
    symlink(chopper_bin(), &chopper_exe).expect("create chopper.exe symlink");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["-V", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_exe,
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_chopper_cmd_fall_back_to_alias_validation_error() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("chopper.cmd");
    symlink(chopper_bin(), &chopper_cmd).expect("create chopper.cmd symlink");

    let output = run_chopper_with(
        chopper_cmd.clone(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_cmd.clone(),
        &config_home,
        &cache_home,
        &["--version", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_cmd.clone(),
        &config_home,
        &cache_home,
        &["-V", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_chopper_bat_fall_back_to_alias_validation_error() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("chopper.bat");
    symlink(chopper_bin(), &chopper_bat).expect("create chopper.bat symlink");

    let output = run_chopper_with(
        chopper_bat.clone(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_bat.clone(),
        &config_home,
        &cache_home,
        &["--version", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_bat.clone(),
        &config_home,
        &cache_home,
        &["-V", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_uppercase_chopper_bat_fall_back_to_alias_validation_error() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("CHOPPER.BAT");
    symlink(chopper_bin(), &chopper_bat).expect("create CHOPPER.BAT symlink");

    let output = run_chopper_with(
        chopper_bat.clone(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_chopper_com_fall_back_to_alias_validation_error() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("chopper.com");
    symlink(chopper_bin(), &chopper_com).expect("create chopper.com symlink");

    let output = run_chopper_with(
        chopper_com.clone(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_uppercase_chopper_cmd_fall_back_to_alias_validation_error() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("CHOPPER.CMD");
    symlink(chopper_bin(), &chopper_cmd).expect("create CHOPPER.CMD symlink");

    let output = run_chopper_with(
        chopper_cmd.clone(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_uppercase_chopper_com_fall_back_to_alias_validation_error() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("CHOPPER.COM");
    symlink(chopper_bin(), &chopper_com).expect("create CHOPPER.COM symlink");

    let output = run_chopper_with(
        chopper_com.clone(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_uppercase_chopper_exe_fall_back_to_alias_validation_error() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("CHOPPER.EXE");
    symlink(chopper_bin(), &chopper_exe).expect("create CHOPPER.EXE symlink");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["-h", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["--version", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["-V", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        chopper_exe,
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_windows_relative_uppercase_chopper_bat_fall_back_to_alias_validation_error(
) {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = ".\\CHOPPER.BAT";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_parent_windows_relative_uppercase_chopper_cmd_fall_back_to_alias_validation_error(
) {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "..\\CHOPPER.CMD";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_unc_windows_uppercase_chopper_bat_fall_back_to_alias_validation_error(
) {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "\\\\server\\tools\\CHOPPER.BAT";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_drive_windows_uppercase_chopper_com_fall_back_to_alias_validation_error(
) {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "C:\\tools\\CHOPPER.COM";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_unix_relative_uppercase_chopper_bat_fall_back_to_alias_validation_error(
) {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "./CHOPPER.BAT";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_forward_slash_unc_uppercase_chopper_cmd_fall_back_to_alias_validation_error(
) {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "//server/tools/CHOPPER.CMD";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_mixed_separator_unc_uppercase_chopper_com_fall_back_to_alias_validation_error(
) {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "\\\\server/tools\\CHOPPER.COM";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");
}

#[test]
fn builtin_flags_with_extra_args_via_uppercase_chopper_fall_back_to_alias_validation_error() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let uppercase_chopper = bin_dir.path().join("CHOPPER");
    symlink(chopper_bin(), &uppercase_chopper).expect("create CHOPPER symlink");

    let output = run_chopper_with(
        uppercase_chopper.clone(),
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        uppercase_chopper.clone(),
        &config_home,
        &cache_home,
        &["-h", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        uppercase_chopper.clone(),
        &config_home,
        &cache_home,
        &["--version", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        uppercase_chopper.clone(),
        &config_home,
        &cache_home,
        &["-V", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        uppercase_chopper.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot start with `-`"), "{stderr}");

    let output = run_chopper_with(
        uppercase_chopper,
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
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
fn print_dir_builtins_work_when_invoked_as_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("chopper.exe");
    symlink(chopper_bin(), &chopper_exe).expect("create chopper.exe symlink");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via chopper.exe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with(
        chopper_exe,
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via chopper.exe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("chopper.cmd");
    symlink(chopper_bin(), &chopper_cmd).expect("create chopper.cmd symlink");

    let output = run_chopper_with(
        chopper_cmd.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via chopper.cmd failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via chopper.cmd failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("chopper.bat");
    symlink(chopper_bin(), &chopper_bat).expect("create chopper.bat symlink");

    let output = run_chopper_with(
        chopper_bat.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via chopper.bat failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via chopper.bat failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("chopper.com");
    symlink(chopper_bin(), &chopper_com).expect("create chopper.com symlink");

    let output = run_chopper_with(
        chopper_com.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via chopper.com failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via chopper.com failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_cmd = bin_dir.path().join("CHOPPER.CMD");
    symlink(chopper_bin(), &chopper_cmd).expect("create CHOPPER.CMD symlink");

    let output = run_chopper_with(
        chopper_cmd.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via CHOPPER.CMD failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with(
        chopper_cmd,
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via CHOPPER.CMD failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_uppercase_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_bat = bin_dir.path().join("CHOPPER.BAT");
    symlink(chopper_bin(), &chopper_bat).expect("create CHOPPER.BAT symlink");

    let output = run_chopper_with(
        chopper_bat.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via CHOPPER.BAT failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with(
        chopper_bat,
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via CHOPPER.BAT failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_com = bin_dir.path().join("CHOPPER.COM");
    symlink(chopper_bin(), &chopper_com).expect("create CHOPPER.COM symlink");

    let output = run_chopper_with(
        chopper_com.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with(
        chopper_com,
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_uppercase_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let chopper_exe = bin_dir.path().join("CHOPPER.EXE");
    symlink(chopper_bin(), &chopper_exe).expect("create CHOPPER.EXE symlink");

    let output = run_chopper_with(
        chopper_exe.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via CHOPPER.EXE failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with(
        chopper_exe,
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via CHOPPER.EXE failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_windows_relative_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = ".\\CHOPPER.COM";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via windows-relative CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via windows-relative CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_parent_windows_relative_uppercase_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "..\\CHOPPER.BAT";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via parent windows-relative CHOPPER.BAT failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via parent windows-relative CHOPPER.BAT failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_unc_windows_uppercase_chopper_cmd() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "\\\\server\\tools\\CHOPPER.CMD";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via UNC windows CHOPPER.CMD failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via UNC windows CHOPPER.CMD failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_drive_windows_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "E:\\tools\\CHOPPER.COM";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via drive windows CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via drive windows CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_unix_parent_relative_uppercase_chopper_exe() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "../CHOPPER.EXE";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via unix parent-relative CHOPPER.EXE failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via unix parent-relative CHOPPER.EXE failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_drive_windows_forward_slash_uppercase_chopper_bat() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "D:/bin/CHOPPER.BAT";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via drive forward-slash CHOPPER.BAT failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via drive forward-slash CHOPPER.BAT failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_mixed_separator_drive_windows_uppercase_chopper_com() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let wrapper_name = "C:/tools\\CHOPPER.COM";

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via mixed-separator drive CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with_cwd_and_argv0(
        chopper_bin(),
        wrapper_name,
        bin_dir.path(),
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via mixed-separator drive CHOPPER.COM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        cache_home.path().join("chopper").display().to_string()
    );
}

#[test]
fn print_dir_builtins_work_when_invoked_as_uppercase_chopper() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let bin_dir = TempDir::new().expect("create bin dir");
    let uppercase_chopper = bin_dir.path().join("CHOPPER");
    symlink(chopper_bin(), &uppercase_chopper).expect("create CHOPPER symlink");

    let output = run_chopper_with(
        uppercase_chopper.clone(),
        &config_home,
        &cache_home,
        &["--print-config-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-config-dir via CHOPPER failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        config_home.path().join("chopper").display().to_string()
    );

    let output = run_chopper_with(
        uppercase_chopper,
        &config_home,
        &cache_home,
        &["--print-cache-dir"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "print-cache-dir via CHOPPER failed: {}",
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
fn symlink_mode_help_with_extra_args_is_passthrough() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("helpextracheck.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("helpextracheck");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["--help", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=--help extra"), "{stdout}");
    assert!(!stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn symlink_mode_does_not_treat_short_help_as_builtin() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("shorthelpcheck.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("shorthelpcheck");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["-h"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=-h"), "{stdout}");
    assert!(!stdout.contains("Usage:"), "{stdout}");
}

#[test]
fn symlink_mode_short_help_with_extra_args_is_passthrough() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("shorthelpextra.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("shorthelpextra");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["-h", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=-h extra"), "{stdout}");
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
fn symlink_mode_print_config_dir_with_extra_args_is_passthrough() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("printconfigextracheck.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("printconfigextracheck");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["--print-config-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=--print-config-dir extra"), "{stdout}");
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
fn symlink_mode_print_cache_dir_with_extra_args_is_passthrough() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("printcacheextracheck.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("printcacheextracheck");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["--print-cache-dir", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=--print-cache-dir extra"), "{stdout}");
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
fn symlink_mode_version_with_extra_args_is_passthrough() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("versionextracheck.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("versionextracheck");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["--version", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=--version extra"), "{stdout}");
    assert!(!stdout.contains(env!("CARGO_PKG_VERSION")), "{stdout}");
}

#[test]
fn symlink_mode_does_not_treat_short_version_as_builtin() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("shortversioncheck.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("shortversioncheck");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["-V"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=-V"), "{stdout}");
    assert!(!stdout.contains(env!("CARGO_PKG_VERSION")), "{stdout}");
}

#[test]
fn symlink_mode_short_version_with_extra_args_is_passthrough() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("shortversionextra.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("shortversionextra");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["-V", "extra"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=-V extra"), "{stdout}");
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
fn symlink_invocation_rejects_dash_prefixed_alias_name() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("-bad-alias");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("alias name cannot start with `-`"),
        "{stderr}"
    );
}

#[test]
fn symlink_invocation_rejects_whitespace_alias_name() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("bad alias");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("alias name cannot contain whitespace"),
        "{stderr}"
    );
}

#[test]
fn symlink_invocation_rejects_separator_alias_name() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("--");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("alias name cannot be `--`"), "{stderr}");
}

#[test]
fn symlink_invocation_rejects_pathlike_alias_name() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("bad\\alias");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["runtime"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("alias name cannot contain path separators"),
        "{stderr}"
    );
}

#[test]
fn symlink_invocation_without_runtime_args_still_executes_alias() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("kpods-noargs.toml"),
        r#"
exec = "echo"
args = ["symlink-noargs"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("kpods-noargs");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &[],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("symlink-noargs"), "{stdout}");
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
        alias_colon.clone(),
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

    let output = run_chopper_with(
        alias_colon,
        &config_home,
        &cache_home,
        &["runtime-c"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "second colon command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("alias=colon runtime-c"), "{stdout}");

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
fn symlink_invocation_separator_preserves_literal_double_dash_payload() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("kpods-literal-separator.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]
"#,
    )
    .expect("write alias config");

    let bin_dir = TempDir::new().expect("create bin dir");
    let symlink_path = bin_dir.path().join("kpods-literal-separator");
    symlink(chopper_bin(), &symlink_path).expect("create symlink to chopper");

    let output = run_chopper_with(
        symlink_path,
        &config_home,
        &cache_home,
        &["--", "--", "--tail=100"],
        std::iter::empty::<(&str, String)>(),
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=base -- --tail=100"), "{stdout}");
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
fn alias_lookup_ignores_directory_candidates_and_falls_back_to_path() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    let aliases_dir = chopper_dir.join("aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::create_dir_all(aliases_dir.join("fallbackcmd.toml")).expect("create directory candidate");

    let fake_bin = TempDir::new().expect("create fake-bin dir");
    let command_path = fake_bin.path().join("fallbackcmd");
    write_executable_script(
        &command_path,
        "#!/usr/bin/env bash\nprintf 'PATH_FALLBACK_DIR_SHADOW=%s\\n' \"$*\"\n",
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
    assert!(
        stdout.contains("PATH_FALLBACK_DIR_SHADOW=runtime"),
        "{stdout}"
    );
}

#[test]
fn alias_lookup_ignores_dangling_symlink_candidates_and_falls_back_to_path() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    let aliases_dir = chopper_dir.join("aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    symlink(
        chopper_dir.join("definitely-missing-target.toml"),
        aliases_dir.join("fallbackcmd.toml"),
    )
    .expect("create dangling symlink candidate");

    let fake_bin = TempDir::new().expect("create fake-bin dir");
    let command_path = fake_bin.path().join("fallbackcmd");
    write_executable_script(
        &command_path,
        "#!/usr/bin/env bash\nprintf 'PATH_FALLBACK_DANGLING_SYMLINK=%s\\n' \"$*\"\n",
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
    assert!(
        stdout.contains("PATH_FALLBACK_DANGLING_SYMLINK=runtime"),
        "{stdout}"
    );
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
fn alias_lookup_accepts_symlinked_toml_file_candidate() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    let aliases_dir = chopper_dir.join("aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    let target = chopper_dir.join("symlink-target.toml");
    fs::write(
        &target,
        r#"
exec = "sh"
args = ["-c", "printf 'SYMLINKED_CONFIG=%s\n' \"$*\"", "_", "base"]
"#,
    )
    .expect("write symlink target config");
    symlink(&target, aliases_dir.join("linkcfg.toml")).expect("create alias symlink config");

    let output = run_chopper(&config_home, &cache_home, &["linkcfg", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SYMLINKED_CONFIG=base runtime"), "{stdout}");
}

#[test]
fn symlinked_alias_config_resolves_reconcile_script_relative_to_target() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    let aliases_dir = chopper_dir.join("aliases");
    let shared_dir = chopper_dir.join("shared");
    let hooks_dir = shared_dir.join("hooks");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::create_dir_all(&hooks_dir).expect("create hooks dir");

    fs::write(
        hooks_dir.join("reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    append_args: ["from_target_relative_script"]
  }
}
"#,
    )
    .expect("write reconcile script");
    let target = shared_dir.join("target.toml");
    fs::write(
        &target,
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]

[reconcile]
script = "hooks/reconcile.rhai"
"#,
    )
    .expect("write symlink target config");
    symlink(&target, aliases_dir.join("linkreconcile.toml")).expect("create alias symlink config");

    let output = run_chopper(&config_home, &cache_home, &["linkreconcile", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ARGS=base runtime from_target_relative_script"),
        "{stdout}"
    );
}

#[test]
fn alias_config_resolves_reconcile_script_relative_to_its_own_directory() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    let hooks_dir = aliases_dir.join("hooks");
    fs::create_dir_all(&hooks_dir).expect("create hooks dir");

    fs::write(
        hooks_dir.join("reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    append_args: ["from_local_relative_script"]
  }
}
"#,
    )
    .expect("write reconcile script");
    fs::write(
        aliases_dir.join("localreconcile.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]

[reconcile]
script = "hooks/reconcile.rhai"
"#,
    )
    .expect("write local reconcile alias config");

    let output = run_chopper(&config_home, &cache_home, &["localreconcile", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ARGS=base runtime from_local_relative_script"),
        "{stdout}"
    );
}

#[test]
fn alias_config_resolves_parent_relative_reconcile_script_from_its_own_directory() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    let aliases_dir = chopper_dir.join("aliases");
    let hooks_dir = chopper_dir.join("hooks");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::create_dir_all(&hooks_dir).expect("create hooks dir");

    fs::write(
        hooks_dir.join("reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    append_args: ["from_parent_relative_script"]
  }
}
"#,
    )
    .expect("write reconcile script");
    fs::write(
        aliases_dir.join("parentreconcile.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]

[reconcile]
script = "../hooks/reconcile.rhai"
"#,
    )
    .expect("write parent reconcile alias config");

    let output = run_chopper(&config_home, &cache_home, &["parentreconcile", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ARGS=base runtime from_parent_relative_script"),
        "{stdout}"
    );
}

#[test]
fn alias_config_resolves_dot_prefixed_reconcile_script_from_its_own_directory() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    let hooks_dir = aliases_dir.join("hooks");
    fs::create_dir_all(&hooks_dir).expect("create hooks dir");

    fs::write(
        hooks_dir.join("reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    append_args: ["from_dot_relative_script"]
  }
}
"#,
    )
    .expect("write reconcile script");
    fs::write(
        aliases_dir.join("dotreconcile.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]

[reconcile]
script = "./hooks/reconcile.rhai"
"#,
    )
    .expect("write dot reconcile alias config");

    let output = run_chopper(&config_home, &cache_home, &["dotreconcile", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ARGS=base runtime from_dot_relative_script"),
        "{stdout}"
    );
}

#[test]
fn symlinked_alias_config_resolves_relative_exec_path_to_target_directory() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    let aliases_dir = chopper_dir.join("aliases");
    let shared_dir = chopper_dir.join("shared");
    let bin_dir = shared_dir.join("bin");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    write_executable_script(
        &bin_dir.join("runner"),
        "#!/usr/bin/env bash\nprintf 'REL_EXEC=%s\\n' \"$*\"\n",
    );
    let target = shared_dir.join("target-exec.toml");
    fs::write(
        &target,
        r#"
exec = "bin/runner"
args = ["base"]
"#,
    )
    .expect("write symlink target config");
    symlink(&target, aliases_dir.join("linkexec.toml")).expect("create alias symlink config");

    let output = run_chopper(&config_home, &cache_home, &["linkexec", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("REL_EXEC=base runtime"), "{stdout}");
}

#[test]
fn alias_config_resolves_relative_exec_path_from_its_own_directory() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    let bin_dir = aliases_dir.join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    write_executable_script(
        &bin_dir.join("runner"),
        "#!/usr/bin/env bash\nprintf 'REL_EXEC_LOCAL=%s\\n' \"$*\"\n",
    );
    fs::write(
        aliases_dir.join("localexec.toml"),
        r#"
exec = "bin/runner"
args = ["base"]
"#,
    )
    .expect("write local exec alias config");

    let output = run_chopper(&config_home, &cache_home, &["localexec", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("REL_EXEC_LOCAL=base runtime"), "{stdout}");
}

#[test]
fn alias_config_resolves_dot_prefixed_relative_exec_path_from_its_own_directory() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    let bin_dir = aliases_dir.join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    write_executable_script(
        &bin_dir.join("runner"),
        "#!/usr/bin/env bash\nprintf 'REL_EXEC_DOT=%s\\n' \"$*\"\n",
    );
    fs::write(
        aliases_dir.join("dotexec.toml"),
        r#"
exec = "./bin/runner"
args = ["base"]
"#,
    )
    .expect("write dot exec alias config");

    let output = run_chopper(&config_home, &cache_home, &["dotexec", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("REL_EXEC_DOT=base runtime"), "{stdout}");
}

#[test]
fn alias_config_resolves_parent_relative_exec_path_from_its_own_directory() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    let aliases_dir = chopper_dir.join("aliases");
    let bin_dir = chopper_dir.join("bin");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    write_executable_script(
        &bin_dir.join("runner"),
        "#!/usr/bin/env bash\nprintf 'REL_EXEC_PARENT=%s\\n' \"$*\"\n",
    );
    fs::write(
        aliases_dir.join("parentexec.toml"),
        r#"
exec = "../bin/runner"
args = ["base"]
"#,
    )
    .expect("write parent relative exec alias config");

    let output = run_chopper(&config_home, &cache_home, &["parentexec", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("REL_EXEC_PARENT=base runtime"), "{stdout}");
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
fn journal_identifier_is_trimmed_before_forwarding() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("journal-id-trimmed.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ERR_STREAM\n' 1>&2"]

[journal]
namespace = "ops-e2e"
stderr = true
identifier = "  id-trimmed  "
"#,
    )
    .expect("write alias config");

    let fake_bin = TempDir::new().expect("create fake-bin dir");
    let captured_args = fake_bin.path().join("captured-args.log");
    let script_path = fake_bin.path().join("systemd-cat");
    write_executable_script(
        &script_path,
        &format!(
            "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > \"{}\"\ncat >/dev/null\n",
            captured_args.display()
        ),
    );

    let existing_path = std::env::var("PATH").unwrap_or_default();
    let merged_path = format!("{}:{existing_path}", fake_bin.path().display());
    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["journal-id-trimmed"],
        [("PATH", merged_path)],
    );

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let captured_args_text =
        fs::read_to_string(&captured_args).expect("read captured systemd-cat args");
    assert!(
        captured_args_text.contains("--identifier=id-trimmed"),
        "trimmed identifier should be forwarded: {captured_args_text}"
    );
    assert!(
        !captured_args_text.contains("--identifier=  id-trimmed  "),
        "identifier should not retain surrounding whitespace: {captured_args_text}"
    );
}

#[test]
fn journal_namespace_with_nul_escape_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("journal-nul-namespace.toml"),
        r#"
exec = "echo"

[journal]
namespace = "ops\u0000prod"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["journal-nul-namespace"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `journal.namespace` cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn journal_identifier_with_nul_escape_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("journal-nul-identifier.toml"),
        r#"
exec = "echo"

[journal]
namespace = "ops"
identifier = "svc\u0000id"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["journal-nul-identifier"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `journal.identifier` cannot contain NUL bytes"),
        "{stderr}"
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
fn unicode_alias_name_executes_and_caches_safely() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    let alias = "emoji🚀";
    fs::write(
        aliases_dir.join(format!("{alias}.toml")),
        r#"
exec = "echo"
args = ["unicode"]
"#,
    )
    .expect("write alias config");

    let first = run_chopper(&config_home, &cache_home, &[alias, "runtime"]);
    assert!(
        first.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let stdout = String::from_utf8_lossy(&first.stdout);
    assert!(stdout.contains("unicode runtime"), "{stdout}");

    let second = run_chopper(&config_home, &cache_home, &[alias, "again"]);
    assert!(
        second.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(stdout.contains("unicode again"), "{stdout}");

    let manifests_dir = cache_home.path().join("chopper/manifests");
    let matching_cache_entries = fs::read_dir(&manifests_dir)
        .expect("read manifest cache dir")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("emoji_") && name.ends_with(".bin"))
        .count();
    assert_eq!(
        matching_cache_entries, 1,
        "expected one cache entry for unicode alias in {:?}",
        manifests_dir
    );
}

#[test]
fn direct_invocation_supports_dotted_alias_identifiers() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("demo.prod.toml"),
        r#"
exec = "echo"
args = ["direct-dot-alias"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["demo.prod", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("direct-dot-alias runtime"), "{stdout}");
}

#[test]
fn direct_aliases_that_sanitize_to_same_cache_prefix_do_not_collide() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("alpha:beta.toml"),
        r#"
exec = "echo"
args = ["direct=colon"]
"#,
    )
    .expect("write colon alias config");
    fs::write(
        aliases_dir.join("alpha?beta.toml"),
        r#"
exec = "echo"
args = ["direct=question"]
"#,
    )
    .expect("write question alias config");

    let output = run_chopper(&config_home, &cache_home, &["alpha:beta", "runtime-a"]);
    assert!(
        output.status.success(),
        "colon command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("direct=colon runtime-a"), "{stdout}");

    let output = run_chopper(&config_home, &cache_home, &["alpha?beta", "runtime-b"]);
    assert!(
        output.status.success(),
        "question command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("direct=question runtime-b"), "{stdout}");

    let output = run_chopper(&config_home, &cache_home, &["alpha:beta", "runtime-c"]);
    assert!(
        output.status.success(),
        "second colon command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("direct=colon runtime-c"), "{stdout}");

    let manifests_dir = cache_home.path().join("chopper/manifests");
    let matching_cache_entries = fs::read_dir(&manifests_dir)
        .expect("read manifests dir")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("alpha_beta-") && name.ends_with(".bin"))
        .count();
    assert_eq!(
        matching_cache_entries, 2,
        "expected one cache entry per colliding-sanitization direct alias in {:?}",
        manifests_dir
    );
}

#[test]
fn unsafe_alias_cache_entry_migrates_from_legacy_filename_on_load() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("alpha:beta.toml"),
        r#"
exec = "echo"
args = ["migration-check"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["alpha:beta", "first-run"]);
    assert!(
        output.status.success(),
        "first run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifests_dir = cache_home.path().join("chopper/manifests");
    let hashed_file = fs::read_dir(&manifests_dir)
        .expect("read manifests dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|name| name.starts_with("alpha_beta-") && name.ends_with(".bin"))
                .unwrap_or(false)
        })
        .expect("expected hashed cache file");
    let legacy_file = manifests_dir.join("alpha_beta.bin");
    fs::rename(&hashed_file, &legacy_file).expect("rename hashed cache file to legacy name");
    assert!(
        !hashed_file.exists(),
        "hashed cache file should be absent after legacy rename"
    );
    assert!(
        legacy_file.exists(),
        "legacy cache file should exist after rename"
    );

    let output = run_chopper(&config_home, &cache_home, &["alpha:beta", "second-run"]);
    assert!(
        output.status.success(),
        "second run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("migration-check second-run"), "{stdout}");
    assert!(
        hashed_file.exists(),
        "hashed cache file should be restored after migration load path"
    );
    assert!(
        !legacy_file.exists(),
        "legacy cache file should be pruned after migration"
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
fn parser_trimming_applies_to_env_keys_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("env-key-trim.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ENV=%s\n' \"$CHOPPER_E2E\""]

[env]
"  CHOPPER_E2E  " = "from_alias"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["env-key-trim"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ENV=from_alias"), "{stdout}");
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
fn direct_invocation_separator_preserves_literal_double_dash_payload() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("kpods-direct-literal-separator.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'ARGS=%s\n' \"$*\"", "_", "base"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["kpods-direct-literal-separator", "--", "--", "--tail=100"],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARGS=base -- --tail=100"), "{stdout}");
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
fn cache_invalidation_applies_when_symlinked_alias_target_changes() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    let aliases_dir = chopper_dir.join("aliases");
    let shared_dir = chopper_dir.join("shared");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    fs::create_dir_all(&shared_dir).expect("create shared dir");

    let target_path = shared_dir.join("mutable-target.toml");
    fs::write(
        &target_path,
        r#"
exec = "echo"
args = ["before-symlink-change"]
"#,
    )
    .expect("write target alias config");
    symlink(&target_path, aliases_dir.join("mutable-link.toml")).expect("create alias symlink");

    let first = run_chopper(&config_home, &cache_home, &["mutable-link"]);
    assert!(
        first.status.success(),
        "first run failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    assert!(
        first_stdout.contains("before-symlink-change"),
        "{first_stdout}"
    );

    fs::write(
        &target_path,
        r#"
exec = "echo"
args = ["after-symlink-change"]
"#,
    )
    .expect("rewrite target alias config");

    let second = run_chopper(&config_home, &cache_home, &["mutable-link"]);
    assert!(
        second.status.success(),
        "second run failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        second_stdout.contains("after-symlink-change"),
        "{second_stdout}"
    );
    assert!(
        !second_stdout.contains("before-symlink-change"),
        "{second_stdout}"
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
fn journal_immediate_sink_exit_before_child_spawn_avoids_side_effects() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");
    let side_effect = config_home.path().join("should-not-exist-early-exit");

    fs::write(
        aliases_dir.join("journal-early-exit.toml"),
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

    let fake_bin = TempDir::new().expect("create fake-bin dir");
    let script_path = fake_bin.path().join("systemd-cat");
    write_executable_script(&script_path, "#!/usr/bin/env bash\nexit 17\n");
    let existing_path = std::env::var("PATH").unwrap_or_default();
    let merged_path = format!("{}:{existing_path}", fake_bin.path().display());

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["journal-early-exit"],
        [("PATH", merged_path)],
    );

    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("exited before child spawn"), "{stderr}");
    assert!(
        !side_effect.exists(),
        "child command should not run when systemd-cat exits immediately"
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
fn reconcile_env_key_and_remove_entries_are_trimmed_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-trim.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    set_env: #{ "  CHOPPER_PROMOTE  ": "from_reconcile" },
    remove_env: ["  CHOPPER_DROP  ", "   "]
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-trim.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'PROMOTE=%s\n' \"$CHOPPER_PROMOTE\"; printf 'DROP=%s\n' \"$CHOPPER_DROP\""]

[reconcile]
script = "reconcile-trim.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["reconcile-trim"],
        [("CHOPPER_DROP", "from_runtime".to_string())],
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PROMOTE=from_reconcile"), "{stdout}");
    assert!(stdout.contains("DROP="), "{stdout}");
    assert!(!stdout.contains("DROP=from_runtime"), "{stdout}");
}

#[test]
fn reconcile_remove_env_duplicates_are_deduped_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-remove-dedup.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    remove_env: ["CHOPPER_DROP", " CHOPPER_DROP ", "CHOPPER_DROP"]
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-remove-dedup.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'DROP=%s\n' \"$CHOPPER_DROP\"; printf 'KEEP=%s\n' \"$CHOPPER_KEEP\""]

[env]
CHOPPER_KEEP = "from_alias"

[reconcile]
script = "reconcile-remove-dedup.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["reconcile-remove-dedup"],
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
fn reconcile_blank_set_env_key_after_trim_fails_validation_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-bad-key.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    set_env: #{ "   ": "bad" }
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-bad-key.toml"),
        r#"
exec = "echo"

[reconcile]
script = "reconcile-bad-key.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["reconcile-bad-key"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`set_env` cannot contain empty keys"),
        "{stderr}"
    );
}

#[test]
fn reconcile_duplicate_set_env_keys_after_trim_fail_validation_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-dup-key.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    set_env: #{ "CHOPPER_DUP": "a", " CHOPPER_DUP ": "b" }
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-dup-key.toml"),
        r#"
exec = "echo"

[reconcile]
script = "reconcile-dup-key.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["reconcile-dup-key"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`set_env` contains duplicate keys after trimming"),
        "{stderr}"
    );
}

#[test]
fn reconcile_set_env_key_with_equals_sign_fails_validation_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-equals-key.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    set_env: #{ "BAD=KEY": "value" }
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-equals-key.toml"),
        r#"
exec = "echo"

[reconcile]
script = "reconcile-equals-key.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["reconcile-equals-key"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`set_env` keys cannot contain `=`"),
        "{stderr}"
    );
}

#[test]
fn reconcile_set_env_key_with_nul_byte_fails_validation_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-nul-key.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  let bad_key = "BAD" + "\x00" + "KEY";
  let env = #{};
  env[bad_key] = "value";
  #{
    set_env: env
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-nul-key.toml"),
        r#"
exec = "echo"

[reconcile]
script = "reconcile-nul-key.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["reconcile-nul-key"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`set_env` keys cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn reconcile_set_env_value_with_nul_byte_fails_validation_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-nul-value.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    set_env: #{ "GOOD_KEY": "bad\x00value" }
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-nul-value.toml"),
        r#"
exec = "echo"

[reconcile]
script = "reconcile-nul-value.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["reconcile-nul-value"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`set_env` values cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn reconcile_append_args_with_nul_byte_fails_validation_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-append-nul.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    append_args: ["ok", "bad\x00arg"]
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-append-nul.toml"),
        r#"
exec = "echo"

[reconcile]
script = "reconcile-append-nul.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["reconcile-append-nul"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`append_args` entries cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn reconcile_replace_args_with_nul_byte_fails_validation_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-replace-nul.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    replace_args: ["ok", "bad\x00arg"]
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-replace-nul.toml"),
        r#"
exec = "echo"

[reconcile]
script = "reconcile-replace-nul.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["reconcile-replace-nul"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`replace_args` entries cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn reconcile_remove_env_entry_with_equals_sign_fails_validation_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-remove-equals.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    remove_env: ["BAD=KEY"]
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-remove-equals.toml"),
        r#"
exec = "echo"

[reconcile]
script = "reconcile-remove-equals.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["reconcile-remove-equals"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`remove_env` entries cannot contain `=`"),
        "{stderr}"
    );
}

#[test]
fn reconcile_remove_env_entry_with_nul_byte_fails_validation_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-remove-nul.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  let bad_key = "BAD" + "\x00" + "KEY";
  #{
    remove_env: [bad_key]
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-remove-nul.toml"),
        r#"
exec = "echo"

[reconcile]
script = "reconcile-remove-nul.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["reconcile-remove-nul"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`remove_env` entries cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn reconcile_unsupported_patch_key_fails_validation_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("reconcile-unsupported-key.reconcile.rhai"),
        r#"
fn reconcile(_ctx) {
  #{
    bogus_key: "value"
  }
}
"#,
    )
    .expect("write reconcile script");

    fs::write(
        aliases_dir.join("reconcile-unsupported-key.toml"),
        r#"
exec = "echo"

[reconcile]
script = "reconcile-unsupported-key.reconcile.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["reconcile-unsupported-key"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported reconcile patch key `bogus_key`"),
        "{stderr}"
    );
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
fn env_remove_duplicates_are_deduped_in_end_to_end_flow() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("envremove-dedup.toml"),
        r#"
exec = "sh"
args = ["-c", "printf 'DROP=%s\n' \"$CHOPPER_DROP\"; printf 'KEEP=%s\n' \"$CHOPPER_KEEP\""]
env_remove = ["CHOPPER_DROP", " CHOPPER_DROP ", "CHOPPER_DROP"]

[env]
CHOPPER_KEEP = "from_alias"
"#,
    )
    .expect("write alias config");

    let output = run_chopper_with(
        chopper_bin(),
        &config_home,
        &cache_home,
        &["envremove-dedup"],
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
fn env_remove_entry_with_equals_sign_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("envremove-invalid.toml"),
        r#"
exec = "echo"
env_remove = ["BAD=KEY"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["envremove-invalid"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `env_remove` entries cannot contain `=`"),
        "{stderr}"
    );
}

#[test]
fn env_remove_entry_with_nul_escape_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("envremove-nul.toml"),
        r#"
exec = "echo"
env_remove = ["BAD\u0000KEY"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["envremove-nul"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `env_remove` entries cannot contain NUL bytes"),
        "{stderr}"
    );
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
fn legacy_one_line_alias_ignores_blank_and_comment_lines() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(
        chopper_dir.join("legacy-comments"),
        r#"

# heading comment
   # indented comment
echo legacy-commented
"#,
    )
    .expect("write legacy alias");

    let output = run_chopper(&config_home, &cache_home, &["legacy-comments", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("legacy-commented runtime"), "{stdout}");
}

#[test]
fn legacy_one_line_alias_accepts_utf8_bom_prefixed_command() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(chopper_dir.join("legacy-bom"), "\u{feff}echo bom-ok").expect("write legacy alias");

    let output = run_chopper(&config_home, &cache_home, &["legacy-bom", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("bom-ok runtime"), "{stdout}");
}

#[test]
fn legacy_one_line_alias_with_nul_in_command_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(chopper_dir.join("legacy-nul-command"), b"ec\0ho legacy")
        .expect("write legacy alias");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["legacy-nul-command", "runtime"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("legacy alias command cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn legacy_one_line_alias_with_nul_in_args_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(chopper_dir.join("legacy-nul-args"), b"echo ok\0bad").expect("write legacy alias");

    let output = run_chopper(&config_home, &cache_home, &["legacy-nul-args", "runtime"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("legacy alias args entries cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn legacy_one_line_alias_with_empty_command_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(chopper_dir.join("legacy-empty-command"), "\"\" runtime")
        .expect("write legacy alias");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["legacy-empty-command", "runtime"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("legacy alias command cannot be empty"),
        "{stderr}"
    );
}

#[test]
fn legacy_one_line_alias_with_dot_command_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(chopper_dir.join("legacy-dot-command"), ". runtime").expect("write legacy alias");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["legacy-dot-command", "runtime"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("legacy alias command cannot be `.` or `..`"),
        "{stderr}"
    );
}

#[test]
fn legacy_one_line_alias_with_parent_command_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(chopper_dir.join("legacy-parent-command"), ".. runtime").expect("write legacy alias");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["legacy-parent-command", "runtime"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("legacy alias command cannot be `.` or `..`"),
        "{stderr}"
    );
}

#[test]
fn legacy_one_line_alias_with_trailing_separator_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(
        chopper_dir.join("legacy-trailing-separator"),
        "bin/ runtime",
    )
    .expect("write legacy alias");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["legacy-trailing-separator", "runtime"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("legacy alias command cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn legacy_one_line_alias_with_trailing_backslash_separator_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(
        chopper_dir.join("legacy-trailing-backslash-separator"),
        "'bin\\' runtime",
    )
    .expect("write legacy alias");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["legacy-trailing-backslash-separator", "runtime"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("legacy alias command cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn legacy_one_line_alias_with_trailing_dot_component_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(chopper_dir.join("legacy-dot-component"), "bin/.. runtime")
        .expect("write legacy alias");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["legacy-dot-component", "runtime"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("legacy alias command cannot end with `.` or `..` path components"),
        "{stderr}"
    );
}

#[test]
fn legacy_one_line_alias_with_backslash_dot_component_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let chopper_dir = config_home.path().join("chopper");
    fs::create_dir_all(&chopper_dir).expect("create chopper config dir");
    fs::write(
        chopper_dir.join("legacy-backslash-dot-component"),
        "'bin\\..' runtime",
    )
    .expect("write legacy alias");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["legacy-backslash-dot-component", "runtime"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("legacy alias command cannot end with `.` or `..` path components"),
        "{stderr}"
    );
}

#[test]
fn toml_alias_accepts_utf8_bom_prefixed_document() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("bom.toml"),
        "\u{feff}exec = \"sh\"\nargs = [\"-c\", \"printf '%s\\n' \\\"$*\\\"\", \"_\", \"bom\"]\n",
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["bom", "runtime"]);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("bom runtime"), "{stdout}");
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

#[test]
fn toml_env_blank_key_after_trim_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("blank-env-key.toml"),
        r#"
exec = "echo"

[env]
"   " = "value"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["blank-env-key"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `env` cannot contain empty keys"),
        "{stderr}"
    );
}

#[test]
fn toml_env_key_with_equals_sign_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("equals-env-key.toml"),
        r#"
exec = "echo"

[env]
"BAD=KEY" = "value"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["equals-env-key"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `env` keys cannot contain `=`"),
        "{stderr}"
    );
}

#[test]
fn toml_env_key_with_nul_escape_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("nul-env-key.toml"),
        r#"
exec = "echo"

[env]
"BAD\u0000KEY" = "value"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["nul-env-key"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `env` keys cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn toml_env_value_with_nul_escape_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("nul-env-value.toml"),
        r#"
exec = "echo"

[env]
GOOD_KEY = "bad\u0000value"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["nul-env-value"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `env` values cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn toml_args_with_nul_escape_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("nul-args.toml"),
        r#"
exec = "echo"
args = ["ok", "bad\u0000arg"]
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["nul-args"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `args` entries cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn toml_exec_dot_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dot-exec.toml"),
        r#"
exec = "."
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dot-exec"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `exec` cannot be `.` or `..`"),
        "{stderr}"
    );
}

#[test]
fn toml_exec_with_nul_escape_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("nul-exec.toml"),
        "exec = \"echo\\u0000tool\"\n",
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["nul-exec"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `exec` cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn toml_exec_parent_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("parent-exec.toml"),
        r#"
exec = ".."
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["parent-exec"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `exec` cannot be `.` or `..`"),
        "{stderr}"
    );
}

#[test]
fn toml_exec_path_ending_in_dot_component_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dot-component-exec.toml"),
        r#"
exec = "bin/.."
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dot-component-exec"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `exec` cannot end with `.` or `..` path components"),
        "{stderr}"
    );
}

#[test]
fn toml_exec_absolute_path_ending_in_dot_component_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dot-component-absolute-exec.toml"),
        r#"
exec = "/usr/bin/.."
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dot-component-absolute-exec"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `exec` cannot end with `.` or `..` path components"),
        "{stderr}"
    );
}

#[test]
fn toml_exec_dot_slash_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dot-slash-exec.toml"),
        r#"
exec = "./"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dot-slash-exec"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `exec` cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn toml_exec_dot_backslash_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dot-backslash-exec.toml"),
        r#"
exec = '.\'
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dot-backslash-exec"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `exec` cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn toml_exec_trailing_separator_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("trailing-exec.toml"),
        r#"
exec = "./bin/"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["trailing-exec"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `exec` cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn toml_exec_absolute_trailing_separator_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("absolute-trailing-exec.toml"),
        r#"
exec = "/usr/bin/"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["absolute-trailing-exec"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `exec` cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn toml_exec_absolute_trailing_backslash_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("absolute-trailing-backslash-exec.toml"),
        r#"
exec = '/usr/bin\'
"#,
    )
    .expect("write alias config");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["absolute-trailing-backslash-exec"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `exec` cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_script_dot_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dot-reconcile.toml"),
        r#"
exec = "echo"

[reconcile]
script = "."
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dot-reconcile"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.script` cannot be `.` or `..`"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_script_with_nul_escape_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("nul-reconcile-script.toml"),
        r#"
exec = "echo"

[reconcile]
script = "hooks/reconcile\u0000.rhai"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["nul-reconcile-script"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.script` cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_function_with_nul_escape_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("nul-reconcile-function.toml"),
        r#"
exec = "echo"

[reconcile]
script = "hooks/reconcile.rhai"
function = "reconcile\u0000hook"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["nul-reconcile-function"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.function` cannot contain NUL bytes"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_script_parent_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("parent-reconcile.toml"),
        r#"
exec = "echo"

[reconcile]
script = ".."
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["parent-reconcile"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.script` cannot be `.` or `..`"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_script_ending_in_dot_component_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dot-component-reconcile.toml"),
        r#"
exec = "echo"

[reconcile]
script = "hooks/.."
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dot-component-reconcile"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.script` cannot end with `.` or `..` path components"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_script_absolute_path_ending_in_dot_component_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dot-component-absolute-reconcile.toml"),
        r#"
exec = "echo"

[reconcile]
script = "/tmp/.."
"#,
    )
    .expect("write alias config");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["dot-component-absolute-reconcile"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.script` cannot end with `.` or `..` path components"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_script_dot_slash_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dot-slash-reconcile.toml"),
        r#"
exec = "echo"

[reconcile]
script = "./"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dot-slash-reconcile"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.script` cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_script_dot_backslash_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("dot-backslash-reconcile.toml"),
        r#"
exec = "echo"

[reconcile]
script = '.\'
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["dot-backslash-reconcile"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.script` cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_script_trailing_separator_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("trailing-reconcile.toml"),
        r#"
exec = "echo"

[reconcile]
script = "./hooks/"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["trailing-reconcile"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.script` cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_script_absolute_trailing_separator_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("absolute-trailing-reconcile.toml"),
        r#"
exec = "echo"

[reconcile]
script = "/tmp/"
"#,
    )
    .expect("write alias config");

    let output = run_chopper(&config_home, &cache_home, &["absolute-trailing-reconcile"]);
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.script` cannot end with a path separator"),
        "{stderr}"
    );
}

#[test]
fn toml_reconcile_script_absolute_trailing_backslash_path_fails_validation() {
    let config_home = TempDir::new().expect("create config home");
    let cache_home = TempDir::new().expect("create cache home");
    let aliases_dir = config_home.path().join("chopper/aliases");
    fs::create_dir_all(&aliases_dir).expect("create aliases dir");

    fs::write(
        aliases_dir.join("absolute-trailing-backslash-reconcile.toml"),
        r#"
exec = "echo"

[reconcile]
script = '/tmp\'
"#,
    )
    .expect("write alias config");

    let output = run_chopper(
        &config_home,
        &cache_home,
        &["absolute-trailing-backslash-reconcile"],
    );
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("field `reconcile.script` cannot end with a path separator"),
        "{stderr}"
    );
}
