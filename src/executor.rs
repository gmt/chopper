use crate::arg_validation::{self, ArgViolation};
use crate::env_validation::{self, EnvKeyViolation, EnvValueViolation};
use crate::manifest::{Invocation, JournalConfig};
use anyhow::{anyhow, Context, Result};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
    validate_journal_config_for_command(&journal)?;

    let mut journal_cmd = Command::new("systemd-cat");
    journal_cmd.arg(format!("--namespace={}", journal.namespace));
    if let Some(identifier) = journal.identifier {
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

fn validate_journal_config_for_command(journal: &JournalConfig) -> Result<()> {
    let namespace = journal.namespace.trim();
    if namespace.is_empty() {
        return Err(anyhow!("journal namespace cannot be empty"));
    }
    if namespace.contains('\0') {
        return Err(anyhow!("journal namespace cannot contain NUL bytes"));
    }
    if let Some(identifier) = &journal.identifier {
        let identifier = identifier.trim();
        if identifier.is_empty() {
            return Err(anyhow!("journal identifier cannot be blank when provided"));
        }
        if identifier.contains('\0') {
            return Err(anyhow!("journal identifier cannot contain NUL bytes"));
        }
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
    use std::collections::HashMap;
    use std::path::PathBuf;

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
        };

        let err = super::validate_journal_config_for_command(&journal).expect_err("expected error");
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
        };

        let err = super::validate_journal_config_for_command(&journal).expect_err("expected error");
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
        };

        let err = super::validate_journal_config_for_command(&journal).expect_err("expected error");
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
        };

        let err = super::validate_journal_config_for_command(&journal).expect_err("expected error");
        assert!(
            err.to_string()
                .contains("journal identifier cannot contain NUL bytes"),
            "{err}"
        );
    }
}
