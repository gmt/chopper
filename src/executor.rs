use crate::manifest::Manifest;
use anyhow::Result;
use std::os::unix::process::CommandExt;
use std::process::Command;

pub fn run(manifest: Manifest, args: &[String]) -> Result<()> {
    let mut cmd = Command::new(&manifest.exec);

    cmd.args(&manifest.args);
    cmd.args(args);

    for (key, val) in &manifest.env {
        cmd.env(key, val);
    }

    for key in &manifest.env_remove {
        cmd.env_remove(key);
    }

    let err = cmd.exec();
    Err(anyhow::anyhow!("exec failed: {}", err))
}
