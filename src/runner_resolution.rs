use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

pub(crate) const CHOPPER_EXE_PATH_ENV: &str = "CHOPPER_EXE_PATH";

pub(crate) fn resolve_chopper_exe() -> Result<PathBuf> {
    if let Some(path) = env::var_os(CHOPPER_EXE_PATH_ENV).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    let current =
        env::current_exe().context("failed to resolve current chopper executable path")?;
    if let Some(parent) = current.parent() {
        let sibling = parent.join(chopper_exe_file_name());
        if sibling.is_file() {
            return Ok(sibling);
        }
    }

    which::which("chopper-exe").with_context(|| {
        format!(
            "unable to locate chopper-exe; set {CHOPPER_EXE_PATH_ENV} or install chopper-exe on PATH"
        )
    })
}

fn chopper_exe_file_name() -> &'static str {
    if cfg!(windows) {
        "chopper-exe.exe"
    } else {
        "chopper-exe"
    }
}

pub(crate) fn current_exe_for_skip_hint() -> Option<PathBuf> {
    env::current_exe().ok()
}
