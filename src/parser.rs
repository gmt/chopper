use crate::manifest::Manifest;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;

pub fn parse(path: &Path) -> Result<Manifest> {
    let content = fs::read_to_string(path)?;

    if looks_like_rhai(&content) {
        parse_rhai(&content)
    } else {
        parse_trivial(&content)
    }
}

fn looks_like_rhai(content: &str) -> bool {
    let trimmed = content.trim();
    trimmed.contains(';')
        || trimmed.contains("run(")
        || trimmed.contains("env(")
        || trimmed.contains("include(")
        || trimmed.contains("exec(")
}

fn parse_trivial(content: &str) -> Result<Manifest> {
    let line = content
        .lines()
        .next()
        .ok_or_else(|| anyhow!("empty config file"))?
        .trim();

    if line.is_empty() {
        return Err(anyhow!("empty config file"));
    }

    let parts = shell_words::split(line)?;
    if parts.is_empty() {
        return Err(anyhow!("no command found"));
    }

    let exec = which::which(&parts[0]).unwrap_or_else(|_| parts[0].clone().into());

    let args = parts[1..].to_vec();

    Ok(Manifest::simple(exec).with_args(args))
}

fn parse_rhai(_content: &str) -> Result<Manifest> {
    todo!("Rhai config parsing not yet implemented")
}
