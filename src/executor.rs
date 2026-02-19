use crate::arg_validation::{self, ArgViolation};
use crate::env_validation::{self, EnvKeyViolation, EnvValueViolation};
use crate::journal_validation::{self, JournalIdentifierViolation, JournalNamespaceViolation};
use crate::manifest::{Invocation, JournalConfig};
use anyhow::{anyhow, Context, Result};
use std::env;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_JOURNAL_BROKER_CMD: &str = "chopper-journal-broker";

pub fn run(invocation: Invocation) -> Result<()> {
    if let Some(journal) = invocation.journal.clone() {
        if journal.stderr {
            return run_with_journal(invocation, journal);
        }
    }
    run_direct(invocation)
}

fn run_direct(invocation: Invocation) -> Result<()> {
    let mut cmd = command_for_invocation(&invocation)?;
    let err = cmd.exec();
    Err(anyhow!("exec failed: {}", err))
}

fn run_with_journal(invocation: Invocation, journal: JournalConfig) -> Result<()> {
    let (namespace, identifier) = normalize_journal_config_for_command(&journal)?;
    if journal.ensure {
        ensure_journal_namespace_ready(&namespace)?;
    }

    let mut journal_cmd = Command::new("systemd-cat");
    journal_cmd.arg(format!("--namespace={namespace}"));
    if let Some(identifier) = identifier {
        journal_cmd.arg(format!("--identifier={identifier}"));
    }
    journal_cmd.stdin(Stdio::piped());
    journal_cmd.stdout(Stdio::null());
    journal_cmd.stderr(Stdio::inherit());

    let mut journal_child = journal_cmd.spawn().with_context(|| {
        "failed to spawn systemd-cat; is systemd installed and --namespace supported (v256+)?"
    })?;
    let mut journal_stdin = journal_child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("failed to open systemd-cat stdin"))?;
    if let Err(err) = ensure_journal_sink_startup(&mut journal_child) {
        drop(journal_stdin);
        return Err(err);
    }

    let mut child_cmd = command_for_invocation(&invocation)?;
    child_cmd.stderr(Stdio::piped());

    let mut child = match child_cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            drop(journal_stdin);
            let _ = journal_child.wait();
            return Err(err)
                .with_context(|| format!("failed to spawn {}", invocation.exec.display()));
        }
    };
    let mut child_stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            drop(journal_stdin);
            let _ = journal_child.wait();
            return Err(anyhow!("failed to capture child stderr"));
        }
    };

    let pump = thread::spawn(move || -> io::Result<()> {
        io::copy(&mut child_stderr, &mut journal_stdin)?;
        Ok(())
    });

    let child_status = child.wait().context("failed waiting for child process")?;
    let pump_result = pump
        .join()
        .map_err(|_| anyhow!("stderr pump thread panicked"))?;

    let journal_status = journal_child
        .wait()
        .context("failed waiting for systemd-cat process")?;
    if let Err(err) = pump_result {
        if err.kind() != io::ErrorKind::BrokenPipe {
            return Err(err).context("failed piping stderr to systemd-cat");
        }
    }
    if !journal_status.success() {
        return Err(journal_status_error(journal_status));
    }

    exit_like_child(child_status)
}

fn journal_status_error(status: ExitStatus) -> anyhow::Error {
    anyhow!(
        "systemd-cat failed with status {status}; journal namespace requires systemd-cat --namespace support"
    )
}

fn ensure_journal_sink_startup(journal_child: &mut std::process::Child) -> Result<()> {
    const STARTUP_GRACE: Duration = Duration::from_millis(10);
    const POLL_INTERVAL: Duration = Duration::from_millis(1);

    let start = Instant::now();
    while start.elapsed() < STARTUP_GRACE {
        if let Some(status) = journal_child
            .try_wait()
            .context("failed checking initial systemd-cat status")?
        {
            return Err(anyhow!(
                "systemd-cat exited before child spawn with status {status}; journal namespace requires systemd-cat --namespace support"
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}

fn command_for_invocation(invocation: &Invocation) -> Result<Command> {
    validate_exec_path_for_command(invocation)?;
    validate_args_for_command(invocation)?;

    let mut cmd = Command::new(&invocation.exec);
    cmd.args(&invocation.args);

    for (key, val) in &invocation.env {
        validate_env_key_for_command(key)?;
        validate_env_value_for_command(key, val)?;
        cmd.env(key, val);
    }

    for key in &invocation.env_remove {
        validate_env_key_for_command(key)?;
        cmd.env_remove(key);
    }
    Ok(cmd)
}

fn validate_exec_path_for_command(invocation: &Invocation) -> Result<()> {
    if invocation.exec.as_os_str().as_bytes().contains(&0) {
        return Err(anyhow!("execution path cannot contain NUL bytes"));
    }
    Ok(())
}

fn validate_args_for_command(invocation: &Invocation) -> Result<()> {
    for arg in &invocation.args {
        if matches!(
            arg_validation::validate_arg_value(arg),
            Err(ArgViolation::ContainsNul)
        ) {
            return Err(anyhow!("command arguments cannot contain NUL bytes"));
        }
    }
    Ok(())
}

fn validate_env_key_for_command(key: &str) -> Result<()> {
    match env_validation::validate_env_key(key) {
        Ok(()) => Ok(()),
        Err(EnvKeyViolation::ContainsEquals) => {
            Err(anyhow!("environment key `{key}` cannot contain `=`"))
        }
        Err(EnvKeyViolation::ContainsNul) => {
            Err(anyhow!("environment key cannot contain NUL bytes"))
        }
    }
}

fn validate_env_value_for_command(key: &str, value: &str) -> Result<()> {
    match env_validation::validate_env_value(value) {
        Ok(()) => Ok(()),
        Err(EnvValueViolation::ContainsNul) => Err(anyhow!(
            "environment value for `{key}` cannot contain NUL bytes"
        )),
    }
}

fn normalize_journal_config_for_command(
    journal: &JournalConfig,
) -> Result<(String, Option<String>)> {
    let logical_namespace = match journal_validation::normalize_namespace(&journal.namespace) {
        Ok(namespace) => namespace,
        Err(JournalNamespaceViolation::Empty) => {
            return Err(anyhow!("journal namespace cannot be empty"));
        }
        Err(JournalNamespaceViolation::ContainsNul) => {
            return Err(anyhow!("journal namespace cannot contain NUL bytes"));
        }
    };
    let identifier = match journal_validation::normalize_optional_identifier_for_invocation(
        journal.identifier.as_deref(),
    ) {
        Ok(identifier) => identifier,
        Err(JournalIdentifierViolation::Blank) => {
            return Err(anyhow!("journal identifier cannot be blank when provided"));
        }
        Err(JournalIdentifierViolation::ContainsNul) => {
            return Err(anyhow!("journal identifier cannot contain NUL bytes"));
        }
    };
    let namespace = if journal.user_scope {
        derive_user_scoped_namespace(&logical_namespace)?
    } else {
        logical_namespace
    };
    Ok((namespace, identifier))
}

fn derive_user_scoped_namespace(namespace: &str) -> Result<String> {
    let uid = current_effective_uid();
    let username = current_username(uid);
    let user_component = sanitize_namespace_component(&username);
    let namespace_component = sanitize_namespace_component(namespace);
    Ok(format!("u{uid}-{user_component}-{namespace_component}"))
}

fn current_effective_uid() -> u32 {
    env::var("UID")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .or_else(|| {
            command_stdout_trimmed("id", &["-u"]).and_then(|value| value.parse::<u32>().ok())
        })
        .unwrap_or(0)
}

fn current_username(uid: u32) -> String {
    if let Ok(user) = env::var("USER") {
        let trimmed = user.trim();
        if !trimmed.is_empty() && !trimmed.contains('\0') {
            return trimmed.to_string();
        }
    }
    if let Some(username) = command_stdout_trimmed("id", &["-un"]) {
        return username;
    }
    format!("uid{uid}")
}

fn command_stdout_trimmed(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn sanitize_namespace_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut previous_was_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            previous_was_dash = false;
            continue;
        }
        if ch == '.' || ch == '_' || ch == '-' {
            out.push(ch);
            previous_was_dash = ch == '-';
            continue;
        }
        if !previous_was_dash {
            out.push('-');
            previous_was_dash = true;
        }
    }

    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "x".to_string()
    } else {
        trimmed.to_string()
    }
}

fn ensure_journal_namespace_ready(namespace: &str) -> Result<()> {
    let broker = env::var("CHOPPER_JOURNAL_BROKER_CMD")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_JOURNAL_BROKER_CMD.to_string());

    let mut tokens = shell_words::split(&broker)
        .with_context(|| "CHOPPER_JOURNAL_BROKER_CMD must be a valid shell-style command line")?;
    if tokens.is_empty() {
        return Err(anyhow!(
            "CHOPPER_JOURNAL_BROKER_CMD did not resolve to an executable command"
        ));
    }

    let executable = tokens.remove(0);
    let mut broker_cmd = Command::new(&executable);
    broker_cmd.args(tokens);
    broker_cmd.arg("ensure");
    broker_cmd.arg("--namespace");
    broker_cmd.arg(namespace);
    broker_cmd.stdin(Stdio::null());
    broker_cmd.stdout(Stdio::null());
    broker_cmd.stderr(Stdio::inherit());

    let status = broker_cmd.status().with_context(|| {
        format!(
            "failed to spawn journal namespace broker `{executable}`; set CHOPPER_JOURNAL_BROKER_CMD or install `{DEFAULT_JOURNAL_BROKER_CMD}`"
        )
    })?;
    if !status.success() {
        return Err(anyhow!(
            "journal namespace broker `{executable}` failed with status {status} while ensuring namespace `{namespace}`"
        ));
    }
    Ok(())
}

fn exit_like_child(status: ExitStatus) -> Result<()> {
    if status.success() {
        return Ok(());
    }
    if let Some(code) = status.code() {
        std::process::exit(code);
    }
    if let Some(signal) = status.signal() {
        std::process::exit(128 + signal);
    }
    Err(anyhow!("child process terminated without a code/signal"))
}

#[cfg(test)]
mod tests {
    use super::command_for_invocation;
    use crate::manifest::{Invocation, JournalConfig};
    use crate::test_support::ENV_LOCK;
    use std::collections::HashMap;
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn invocation() -> Invocation {
        Invocation {
            exec: PathBuf::from("echo"),
            args: vec!["ok".to_string()],
            env: HashMap::new(),
            env_remove: Vec::new(),
            journal: None,
        }
    }

    #[test]
    fn command_builder_rejects_env_key_with_equals_sign() {
        let mut invocation = invocation();
        invocation
            .env
            .insert("BAD=KEY".to_string(), "value".to_string());

        let err = command_for_invocation(&invocation).expect_err("expected validation error");
        assert!(err.to_string().contains("cannot contain `=`"), "{err}");
    }

    #[test]
    fn command_builder_rejects_env_key_with_nul_byte() {
        let mut invocation = invocation();
        invocation
            .env
            .insert("BAD\0KEY".to_string(), "value".to_string());

        let err = command_for_invocation(&invocation).expect_err("expected validation error");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn command_builder_rejects_env_value_with_nul_byte() {
        let mut invocation = invocation();
        invocation
            .env
            .insert("GOOD_KEY".to_string(), "bad\0value".to_string());

        let err = command_for_invocation(&invocation).expect_err("expected validation error");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn command_builder_rejects_env_remove_key_with_nul_byte() {
        let mut invocation = invocation();
        invocation.env_remove = vec!["BAD\0KEY".to_string()];

        let err = command_for_invocation(&invocation).expect_err("expected validation error");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn command_builder_rejects_env_remove_key_with_equals_sign() {
        let mut invocation = invocation();
        invocation.env_remove = vec!["BAD=KEY".to_string()];

        let err = command_for_invocation(&invocation).expect_err("expected validation error");
        assert!(err.to_string().contains("cannot contain `=`"), "{err}");
    }

    #[test]
    fn command_builder_rejects_argument_with_nul_byte() {
        let mut invocation = invocation();
        invocation.args = vec!["ok".to_string(), "bad\0arg".to_string()];

        let err = command_for_invocation(&invocation).expect_err("expected validation error");
        assert!(
            err.to_string()
                .contains("command arguments cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn command_builder_rejects_exec_path_with_nul_byte() {
        let mut invocation = invocation();
        invocation.exec = PathBuf::from("ec\0ho");

        let err = command_for_invocation(&invocation).expect_err("expected validation error");
        assert!(
            err.to_string()
                .contains("execution path cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn journal_validation_rejects_empty_namespace() {
        let journal = JournalConfig {
            namespace: "   ".to_string(),
            stderr: true,
            identifier: None,
            user_scope: false,
            ensure: false,
        };

        let err =
            super::normalize_journal_config_for_command(&journal).expect_err("expected error");
        assert!(
            err.to_string()
                .contains("journal namespace cannot be empty"),
            "{err}"
        );
    }

    #[test]
    fn journal_validation_rejects_nul_namespace() {
        let journal = JournalConfig {
            namespace: "ops\0prod".to_string(),
            stderr: true,
            identifier: None,
            user_scope: false,
            ensure: false,
        };

        let err =
            super::normalize_journal_config_for_command(&journal).expect_err("expected error");
        assert!(
            err.to_string()
                .contains("journal namespace cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn journal_validation_rejects_blank_identifier() {
        let journal = JournalConfig {
            namespace: "ops".to_string(),
            stderr: true,
            identifier: Some("   ".to_string()),
            user_scope: false,
            ensure: false,
        };

        let err =
            super::normalize_journal_config_for_command(&journal).expect_err("expected error");
        assert!(
            err.to_string()
                .contains("journal identifier cannot be blank when provided"),
            "{err}"
        );
    }

    #[test]
    fn journal_validation_rejects_nul_identifier() {
        let journal = JournalConfig {
            namespace: "ops".to_string(),
            stderr: true,
            identifier: Some("svc\0id".to_string()),
            user_scope: false,
            ensure: false,
        };

        let err =
            super::normalize_journal_config_for_command(&journal).expect_err("expected error");
        assert!(
            err.to_string()
                .contains("journal identifier cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn user_scoped_namespace_derivation_prefixes_uid_and_sanitizes_components() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        env::set_var("USER", "User Name+Ops");
        let namespace =
            super::derive_user_scoped_namespace("ops/ns.prod@2026").expect("derive namespace");
        let uid = super::current_effective_uid();
        assert_eq!(namespace, format!("u{uid}-user-name-ops-ops-ns.prod-2026"));
        env::remove_var("USER");
    }

    #[test]
    fn broker_preflight_invokes_configured_broker_command() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let temp = TempDir::new().expect("tempdir");
        let script = temp.path().join("broker.sh");
        let captured = temp.path().join("captured.log");
        write_executable_script(
            &script,
            &format!(
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > \"{}\"\n",
                captured.display()
            ),
        );
        env::set_var(
            "CHOPPER_JOURNAL_BROKER_CMD",
            format!("{} --token abc", script.display()),
        );

        super::ensure_journal_namespace_ready("u1000-me-ops").expect("broker preflight");

        let captured_text = fs::read_to_string(&captured).expect("read captured args");
        assert!(captured_text.contains("--token"), "{captured_text}");
        assert!(captured_text.contains("abc"), "{captured_text}");
        assert!(captured_text.contains("ensure"), "{captured_text}");
        assert!(captured_text.contains("--namespace"), "{captured_text}");
        assert!(captured_text.contains("u1000-me-ops"), "{captured_text}");
        env::remove_var("CHOPPER_JOURNAL_BROKER_CMD");
    }

    #[test]
    fn broker_preflight_surfaces_nonzero_exit() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let temp = TempDir::new().expect("tempdir");
        let script = temp.path().join("broker-fail.sh");
        write_executable_script(&script, "#!/usr/bin/env bash\nexit 23\n");
        env::set_var("CHOPPER_JOURNAL_BROKER_CMD", script.display().to_string());

        let err = super::ensure_journal_namespace_ready("u1000-me-ops")
            .expect_err("non-zero broker should fail");
        assert!(err.to_string().contains("failed with status"), "{err}");
        env::remove_var("CHOPPER_JOURNAL_BROKER_CMD");
    }

    fn write_executable_script(path: &Path, body: &str) {
        fs::write(path, body).expect("write script");
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("set perms");
    }
}
