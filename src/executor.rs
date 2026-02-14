use crate::manifest::{Invocation, JournalConfig};
use anyhow::{anyhow, Context, Result};
use std::io;
use std::os::unix::process::CommandExt;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;

pub fn run(invocation: Invocation) -> Result<()> {
    if let Some(journal) = invocation.journal.clone() {
        if journal.stderr {
            return run_with_journal(invocation, journal);
        }
    }
    run_direct(invocation)
}

fn run_direct(invocation: Invocation) -> Result<()> {
    let mut cmd = command_for_invocation(&invocation);
    let err = cmd.exec();
    Err(anyhow!("exec failed: {}", err))
}

fn run_with_journal(invocation: Invocation, journal: JournalConfig) -> Result<()> {
    let mut child_cmd = command_for_invocation(&invocation);
    child_cmd.stderr(Stdio::piped());

    let mut child = child_cmd
        .spawn()
        .with_context(|| format!("failed to spawn {}", invocation.exec.display()))?;
    let mut child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("failed to capture child stderr"))?;

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

    let pump = thread::spawn(move || -> io::Result<()> {
        io::copy(&mut child_stderr, &mut journal_stdin)?;
        Ok(())
    });

    let child_status = child.wait().context("failed waiting for child process")?;
    pump.join()
        .map_err(|_| anyhow!("stderr pump thread panicked"))?
        .context("failed piping stderr to systemd-cat")?;

    let journal_status = journal_child
        .wait()
        .context("failed waiting for systemd-cat process")?;
    if !journal_status.success() {
        return Err(anyhow!(
            "systemd-cat failed with status {journal_status}; journal namespace requires systemd-cat --namespace support"
        ));
    }

    exit_like_child(child_status)
}

fn command_for_invocation(invocation: &Invocation) -> Command {
    let mut cmd = Command::new(&invocation.exec);
    cmd.args(&invocation.args);

    for (key, val) in &invocation.env {
        cmd.env(key, val);
    }

    for key in &invocation.env_remove {
        cmd.env_remove(key);
    }
    cmd
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
