use anyhow::{anyhow, Context, Result};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

pub fn build_completion_dictionary(api_names: &[&str]) -> String {
    let mut entries = BTreeSet::new();
    for api in api_names {
        if !api.trim().is_empty() {
            entries.insert(api.trim().to_string());
        }
    }
    let mut out = String::new();
    for entry in entries {
        out.push_str(&entry);
        out.push('\n');
    }
    out
}

pub fn build_nvim_bootstrap(dictionary_path: &Path) -> String {
    format!(
        r#"
set nocompatible
syntax on
filetype plugin on
set complete+=k{dictionary}
set completeopt=menu,menuone,noselect

" Best-effort treesitter setup if plugin is available.
lua << EOF
pcall(function()
  require('nvim-treesitter.configs').setup({{
    highlight = {{ enable = true }},
  }})
end)
EOF
"#,
        dictionary = dictionary_path.display()
    )
}

pub fn open_rhai_editor(script_path: &Path, api_names: &[&str]) -> Result<()> {
    if let Some(parent) = script_path.parent() {
        fs_err::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if !script_path.exists() {
        fs_err::write(
            script_path,
            "fn reconcile(ctx) {\n    #{}\n}\n\nfn complete(ctx) {\n    []\n}\n",
        )
        .with_context(|| format!("failed to create {}", script_path.display()))?;
    }

    let cache_dir = crate::cache::cache_dir();
    fs_err::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create {}", cache_dir.display()))?;
    let dict_path = cache_dir.join("rhai-api-completion.dict");
    let dictionary = build_completion_dictionary(api_names);
    fs_err::write(&dict_path, dictionary)
        .with_context(|| format!("failed to write {}", dict_path.display()))?;

    if let Ok(nvim_path) = which::which("nvim") {
        return run_nvim(&nvim_path, script_path, &dict_path);
    }
    if let Ok(vim_path) = which::which("vim") {
        return run_vim(&vim_path, script_path, &dict_path);
    }
    Err(anyhow!(
        "neither `nvim` nor `vim` was found in PATH for Rhai editing"
    ))
}

fn run_nvim(nvim: &Path, script_path: &Path, dict_path: &Path) -> Result<()> {
    let cache_dir = crate::cache::cache_dir();
    let init_path = cache_dir.join("nvim-rhai-bootstrap.vim");
    fs_err::write(&init_path, build_nvim_bootstrap(dict_path))
        .with_context(|| format!("failed to write {}", init_path.display()))?;

    let status = Command::new(nvim)
        .arg("-u")
        .arg(&init_path)
        .arg(script_path)
        .status()
        .with_context(|| format!("failed to launch {}", nvim.display()))?;
    if !status.success() {
        return Err(anyhow!("nvim exited with status {status}"));
    }
    Ok(())
}

fn run_vim(vim: &Path, script_path: &Path, dict_path: &Path) -> Result<()> {
    let complete_cmd = format!("set complete+=k{}", dict_path.display());
    let status = Command::new(vim)
        .arg("-c")
        .arg("syntax on")
        .arg("-c")
        .arg(complete_cmd)
        .arg(script_path)
        .status()
        .with_context(|| format!("failed to launch {}", vim.display()))?;
    if !status.success() {
        return Err(anyhow!("vim exited with status {status}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_completion_dictionary, build_nvim_bootstrap};
    use std::path::Path;

    #[test]
    fn dictionary_builder_deduplicates_and_sorts() {
        let dict = build_completion_dictionary(&["web_fetch", "fs_list", "web_fetch", ""]);
        assert_eq!(dict, "fs_list\nweb_fetch\n");
    }

    #[test]
    fn nvim_bootstrap_contains_dictionary_and_treesitter_block() {
        let bootstrap = build_nvim_bootstrap(Path::new("/tmp/rhai.dict"));
        assert!(bootstrap.contains("/tmp/rhai.dict"));
        assert!(bootstrap.contains("nvim-treesitter"));
    }
}

