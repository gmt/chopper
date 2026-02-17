use anyhow::{anyhow, Context, Result};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TmuxMode {
    Auto,
    On,
    Off,
}

impl TmuxMode {
    pub(crate) fn parse_cli(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "on" | "true" | "yes" | "1" => Some(Self::On),
            "off" | "false" | "no" | "0" => Some(Self::Off),
            _ => None,
        }
    }

    pub(crate) fn as_label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchStrategy {
    Direct,
    TmuxSplit,
    TmuxSession,
}

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

pub fn open_rhai_editor_with_mode(
    script_path: &Path,
    api_names: &[&str],
    tmux_mode: TmuxMode,
) -> Result<()> {
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

    let invocation = build_editor_invocation(script_path, Some(&dict_path))?;
    launch_editor(invocation, tmux_mode)
}

pub fn open_alias_editor(path: &Path, tmux_mode: TmuxMode) -> Result<()> {
    let invocation = build_editor_invocation(path, None)?;
    launch_editor(invocation, tmux_mode)
}

#[derive(Debug)]
struct EditorInvocation {
    program: std::path::PathBuf,
    args: Vec<String>,
}

fn build_editor_invocation(
    path: &Path,
    completion_dict: Option<&Path>,
) -> Result<EditorInvocation> {
    if let Ok(nvim_path) = which::which("nvim") {
        let args = if let Some(dict_path) = completion_dict {
            let cache_dir = crate::cache::cache_dir();
            let init_path = cache_dir.join("nvim-rhai-bootstrap.vim");
            fs_err::write(&init_path, build_nvim_bootstrap(dict_path))
                .with_context(|| format!("failed to write {}", init_path.display()))?;
            vec![
                "-u".to_string(),
                init_path.display().to_string(),
                path.display().to_string(),
            ]
        } else {
            vec![path.display().to_string()]
        };
        return Ok(EditorInvocation {
            program: nvim_path,
            args,
        });
    }

    if let Ok(vim_path) = which::which("vim") {
        let args = if let Some(dict_path) = completion_dict {
            let complete_cmd = format!("set complete+=k{}", dict_path.display());
            vec![
                "-c".to_string(),
                "syntax on".to_string(),
                "-c".to_string(),
                complete_cmd,
                path.display().to_string(),
            ]
        } else {
            vec![path.display().to_string()]
        };
        return Ok(EditorInvocation {
            program: vim_path,
            args,
        });
    }

    Err(anyhow!(
        "neither `nvim` nor `vim` was found in PATH for editing"
    ))
}

fn launch_editor(invocation: EditorInvocation, tmux_mode: TmuxMode) -> Result<()> {
    let tmux_path = which::which("tmux").ok();
    let inside_tmux = inside_tmux_session();
    let has_server = tmux_path
        .as_ref()
        .map(|path| has_tmux_server(path.as_path()))
        .transpose()?
        .unwrap_or(false);
    let strategy = pick_launch_strategy(tmux_mode, tmux_path.is_some(), inside_tmux, has_server)?;

    match strategy {
        LaunchStrategy::Direct => run_direct_editor(&invocation),
        LaunchStrategy::TmuxSplit => run_editor_in_tmux_split(
            tmux_path
                .as_deref()
                .ok_or_else(|| anyhow!("tmux was required but not found in PATH"))?,
            &invocation,
        ),
        LaunchStrategy::TmuxSession => run_editor_in_tmux_session(
            tmux_path
                .as_deref()
                .ok_or_else(|| anyhow!("tmux was required but not found in PATH"))?,
            &invocation,
        ),
    }
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

fn run_editor_in_tmux_split(tmux: &Path, invocation: &EditorInvocation) -> Result<()> {
    let token = wait_token();
    let command = format!(
        "{}; tmux wait-for -S {}",
        shell_command(invocation),
        shell_words::quote(&token)
    );

    let pane_output = Command::new(tmux)
        .args([
            "split-window",
            "-h",
            "-p",
            "50",
            "-P",
            "-F",
            "#{pane_id}",
            &command,
        ])
        .output()
        .with_context(|| format!("failed to launch tmux split via {}", tmux.display()))?;
    if !pane_output.status.success() {
        return Err(anyhow!(
            "tmux split-window failed with status {}",
            pane_output.status
        ));
    }
    let pane_id = String::from_utf8_lossy(&pane_output.stdout)
        .trim()
        .to_string();

    let wait_status = Command::new(tmux)
        .args(["wait-for", &token])
        .status()
        .with_context(|| format!("failed waiting for editor pane via {}", tmux.display()))?;
    if !wait_status.success() {
        return Err(anyhow!("tmux wait-for failed with status {wait_status}"));
    }

    if !pane_id.is_empty() {
        let _ = Command::new(tmux)
            .args(["kill-pane", "-t", &pane_id])
            .status();
    }
    Ok(())
}

fn run_editor_in_tmux_session(tmux: &Path, invocation: &EditorInvocation) -> Result<()> {
    let session = format!("chopper-tui-{}", wait_token());
    let status = Command::new(tmux)
        .args(["new-session", "-s", &session, &shell_command(invocation)])
        .status()
        .with_context(|| format!("failed to launch tmux session via {}", tmux.display()))?;
    if !status.success() {
        return Err(anyhow!("tmux new-session exited with status {status}"));
    }
    Ok(())
}

fn shell_command(invocation: &EditorInvocation) -> String {
    let mut words = Vec::with_capacity(invocation.args.len() + 1);
    words.push(invocation.program.display().to_string());
    words.extend(invocation.args.iter().cloned());
    shell_words::join(words)
}

fn pick_launch_strategy(
    mode: TmuxMode,
    tmux_in_path: bool,
    inside_tmux: bool,
    tmux_server_running: bool,
) -> Result<LaunchStrategy> {
    match mode {
        TmuxMode::Off => Ok(LaunchStrategy::Direct),
        TmuxMode::On => {
            if !tmux_in_path {
                return Err(anyhow!("--tmux=on requires `tmux` in PATH"));
            }
            if inside_tmux {
                Ok(LaunchStrategy::TmuxSplit)
            } else {
                Ok(LaunchStrategy::TmuxSession)
            }
        }
        TmuxMode::Auto => {
            if !tmux_in_path {
                return Ok(LaunchStrategy::Direct);
            }
            if inside_tmux {
                return Ok(LaunchStrategy::TmuxSplit);
            }
            if tmux_server_running {
                // Respect user preference: avoid creating a second detached session.
                return Ok(LaunchStrategy::Direct);
            }
            Ok(LaunchStrategy::TmuxSession)
        }
    }
}

fn inside_tmux_session() -> bool {
    std::env::var_os("TMUX")
        .map(|value| !value.is_empty())
        .unwrap_or(false)
}

fn has_tmux_server(tmux: &Path) -> Result<bool> {
    let status = Command::new(tmux)
        .arg("has-session")
        .status()
        .with_context(|| format!("failed to probe tmux server via {}", tmux.display()))?;
    if status.success() {
        return Ok(true);
    }
    if matches!(status.code(), Some(1)) {
        return Ok(false);
    }
    Ok(false)
}

fn wait_token() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("chopper-edit-{}-{}", std::process::id(), now)
}

#[cfg(test)]
mod tests {
    use super::{
        build_completion_dictionary, build_nvim_bootstrap, pick_launch_strategy, LaunchStrategy,
        TmuxMode,
    };
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

    #[test]
    fn tmux_cli_values_parse_expected_modes() {
        assert_eq!(TmuxMode::parse_cli("auto"), Some(TmuxMode::Auto));
        assert_eq!(TmuxMode::parse_cli("ON"), Some(TmuxMode::On));
        assert_eq!(TmuxMode::parse_cli("off"), Some(TmuxMode::Off));
        assert_eq!(TmuxMode::parse_cli("banana"), None);
    }

    #[test]
    fn launch_strategy_auto_uses_direct_without_tmux() {
        let strategy =
            pick_launch_strategy(TmuxMode::Auto, false, false, false).expect("pick strategy");
        assert_eq!(strategy, LaunchStrategy::Direct);
    }

    #[test]
    fn launch_strategy_auto_prefers_split_inside_tmux() {
        let strategy =
            pick_launch_strategy(TmuxMode::Auto, true, true, false).expect("pick strategy");
        assert_eq!(strategy, LaunchStrategy::TmuxSplit);
    }

    #[test]
    fn launch_strategy_auto_falls_back_when_background_server_exists() {
        let strategy =
            pick_launch_strategy(TmuxMode::Auto, true, false, true).expect("pick strategy");
        assert_eq!(strategy, LaunchStrategy::Direct);
    }

    #[test]
    fn launch_strategy_on_requires_tmux() {
        let err = pick_launch_strategy(TmuxMode::On, false, false, false)
            .expect_err("missing tmux should error in forced mode");
        assert!(err.to_string().contains("requires `tmux`"));
    }
}
