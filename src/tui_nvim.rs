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

pub fn open_rhai_editor_at_method(
    script_path: &Path,
    method_name: &str,
    method_kind: crate::rhai_wiring::RhaiMethodKind,
    api_names: &[&str],
) -> Result<()> {
    crate::rhai_wiring::ensure_method_exists(script_path, method_name, method_kind)?;
    let cache_dir = crate::cache::cache_dir();
    fs_err::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create {}", cache_dir.display()))?;
    let dict_path = cache_dir.join("rhai-api-completion.dict");
    let dictionary = build_completion_dictionary(api_names);
    fs_err::write(&dict_path, dictionary)
        .with_context(|| format!("failed to write {}", dict_path.display()))?;
    let search_command = format!(r"/^\s*fn\s\+{}\s*(\s*ctx\s*)", method_name.trim());
    let invocation = build_editor_invocation(script_path, Some(&dict_path), Some(&search_command))?;
    launch_editor(invocation)
}

#[derive(Debug)]
struct EditorInvocation {
    program: std::path::PathBuf,
    args: Vec<String>,
}

fn build_editor_invocation(
    path: &Path,
    completion_dict: Option<&Path>,
    search_command: Option<&str>,
) -> Result<EditorInvocation> {
    if let Ok(nvim_path) = which::which("nvim") {
        let mut args = if let Some(dict_path) = completion_dict {
            let cache_dir = crate::cache::cache_dir();
            let init_path = cache_dir.join("nvim-rhai-bootstrap.vim");
            fs_err::write(&init_path, build_nvim_bootstrap(dict_path))
                .with_context(|| format!("failed to write {}", init_path.display()))?;
            vec!["-u".to_string(), init_path.display().to_string()]
        } else {
            Vec::new()
        };
        if let Some(search_command) = search_command {
            args.push(format!("+{search_command}"));
        }
        args.push(path.display().to_string());
        return Ok(EditorInvocation {
            program: nvim_path,
            args,
        });
    }

    if let Ok(vim_path) = which::which("vim") {
        let mut args = if let Some(dict_path) = completion_dict {
            let complete_cmd = format!("set complete+=k{}", dict_path.display());
            vec![
                "-c".to_string(),
                "syntax on".to_string(),
                "-c".to_string(),
                complete_cmd,
            ]
        } else {
            Vec::new()
        };
        if let Some(search_command) = search_command {
            args.push("-c".to_string());
            args.push(search_command.to_string());
        }
        args.push(path.display().to_string());
        return Ok(EditorInvocation {
            program: vim_path,
            args,
        });
    }

    Err(anyhow!(
        "neither `nvim` nor `vim` was found in PATH for editing"
    ))
}

fn launch_editor(invocation: EditorInvocation) -> Result<()> {
    run_direct_editor(&invocation)
}

fn run_direct_editor(invocation: &EditorInvocation) -> Result<()> {
    let status = Command::new(&invocation.program)
        .args(&invocation.args)
        .status()
        .with_context(|| format!("failed to launch {}", invocation.program.display()))?;
    if !status.success() {
        return Err(anyhow!(
            "{} exited with status {status}",
            invocation.program.display()
        ));
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
