use anyhow::Context;
use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::style::Stylize;
use crossterm::terminal::{self, ClearType};
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

const SPLIT_MIN_WIDTH: u16 = 100;
const SPLIT_MIN_HEIGHT: u16 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TuiOptions {
    pub(crate) tmux_mode: crate::tui_nvim::TmuxMode,
}

impl Default for TuiOptions {
    fn default() -> Self {
        Self {
            tmux_mode: crate::tui_nvim::TmuxMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutKind {
    Split,
    Modal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneFocus {
    List,
    Inspector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopAction {
    Continue,
    Refresh,
    EditSelected,
    EditReconcileScript,
    Quit,
}

#[derive(Debug)]
struct AppState {
    aliases: Vec<String>,
    selected: usize,
    scroll: usize,
    focus: PaneFocus,
    layout: LayoutKind,
    show_help: bool,
    status_message: String,
    tmux_mode: crate::tui_nvim::TmuxMode,
}

pub fn run_tui(options: TuiOptions) -> i32 {
    match run_tui_inner(options) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
}

fn run_tui_inner(options: TuiOptions) -> anyhow::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!("--tui requires an interactive terminal");
    }

    let aliases = crate::alias_admin::discover_aliases().context("failed to load aliases")?;
    let mut state = AppState {
        aliases,
        selected: 0,
        scroll: 0,
        focus: PaneFocus::List,
        layout: LayoutKind::Modal,
        show_help: false,
        status_message: String::from("Ready"),
        tmux_mode: options.tmux_mode,
    };

    let _guard = TerminalGuard::new()?;
    loop {
        let (width, height) = terminal::size().context("failed to read terminal size")?;
        state.layout = layout_for_size(width, height);
        let status_lines = status_hint_lines(width, state.layout);
        let content_height = content_height(height, status_lines.len() as u16);
        let alias_rows = alias_viewport_rows(state.layout, content_height);
        ensure_selection_visible(&mut state, alias_rows);

        render(&state, width, height, &status_lines)?;

        let event = event::read().context("failed to read keyboard event")?;
        let action = handle_event(&mut state, event, alias_rows);
        match action {
            LoopAction::Continue => {}
            LoopAction::Refresh => {
                refresh_aliases(&mut state)?;
            }
            LoopAction::EditSelected => {
                edit_selected_alias(&mut state)?;
            }
            LoopAction::EditReconcileScript => {
                edit_selected_reconcile_script(&mut state)?;
            }
            LoopAction::Quit => break,
        }
    }

    Ok(())
}

fn handle_event(state: &mut AppState, event: Event, list_height: usize) -> LoopAction {
    match event {
        Event::Key(key) => handle_key_event(state, key, list_height),
        Event::Resize(_, _) => LoopAction::Continue,
        _ => LoopAction::Continue,
    }
}

fn handle_key_event(state: &mut AppState, key: KeyEvent, list_height: usize) -> LoopAction {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return LoopAction::Continue;
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => LoopAction::Quit,
        KeyCode::Up | KeyCode::Char('k') => {
            if state.selected > 0 {
                state.selected -= 1;
            }
            ensure_selection_visible(state, list_height);
            LoopAction::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.selected + 1 < state.aliases.len() {
                state.selected += 1;
            }
            ensure_selection_visible(state, list_height);
            LoopAction::Continue
        }
        KeyCode::Home | KeyCode::Char('g') => {
            state.selected = 0;
            ensure_selection_visible(state, list_height);
            LoopAction::Continue
        }
        KeyCode::End | KeyCode::Char('G') => {
            state.selected = state.aliases.len().saturating_sub(1);
            ensure_selection_visible(state, list_height);
            LoopAction::Continue
        }
        KeyCode::Char('h') | KeyCode::Left => {
            state.focus = PaneFocus::List;
            LoopAction::Continue
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if state.layout == LayoutKind::Split {
                state.focus = PaneFocus::Inspector;
            }
            LoopAction::Continue
        }
        KeyCode::Char('?') => {
            state.show_help = !state.show_help;
            LoopAction::Continue
        }
        KeyCode::Char('r') => LoopAction::Refresh,
        KeyCode::Char('e') => LoopAction::EditReconcileScript,
        KeyCode::Enter => LoopAction::EditSelected,
        _ => LoopAction::Continue,
    }
}

fn refresh_aliases(state: &mut AppState) -> anyhow::Result<()> {
    let previously_selected = state.aliases.get(state.selected).cloned();
    state.aliases = crate::alias_admin::discover_aliases().context("failed to refresh aliases")?;
    state.selected = previously_selected
        .as_deref()
        .and_then(|alias| state.aliases.iter().position(|value| value == alias))
        .unwrap_or(0);
    state.scroll = 0;
    state.status_message = format!("Refreshed {} alias(es)", state.aliases.len());
    Ok(())
}

fn edit_selected_alias(state: &mut AppState) -> anyhow::Result<()> {
    let Some(alias) = state.aliases.get(state.selected).cloned() else {
        state.status_message = String::from("No alias selected");
        return Ok(());
    };
    let Some(path) = resolve_alias_path(&alias) else {
        state.status_message = format!("No config file found for alias `{alias}`");
        return Ok(());
    };

    let result = pause_terminal_for_subprocess(|| {
        crate::tui_nvim::open_alias_editor(&path, state.tmux_mode)
            .with_context(|| format!("failed to open editor for alias `{alias}`"))
    });

    match result {
        Ok(()) => {
            state.status_message = format!("Edited `{alias}`");
            refresh_aliases(state)?;
        }
        Err(err) => {
            state.status_message = err.to_string();
        }
    }
    Ok(())
}

fn edit_selected_reconcile_script(state: &mut AppState) -> anyhow::Result<()> {
    let Some(alias) = state.aliases.get(state.selected).cloned() else {
        state.status_message = String::from("No alias selected");
        return Ok(());
    };
    let Some(config_path) = resolve_alias_path(&alias) else {
        state.status_message = format!("No config file found for alias `{alias}`");
        return Ok(());
    };

    let manifest = match crate::parser::parse(&config_path) {
        Ok(manifest) => manifest,
        Err(err) => {
            state.status_message = format!("Unable to parse alias `{alias}`: {err}");
            return Ok(());
        }
    };
    let Some(reconcile) = manifest.reconcile else {
        state.status_message = format!("Alias `{alias}` has no reconcile script configured");
        return Ok(());
    };

    let result = pause_terminal_for_subprocess(|| {
        crate::tui_nvim::open_rhai_editor_with_mode(
            &reconcile.script,
            &crate::rhai_api_catalog::exported_api_names(),
            state.tmux_mode,
        )
        .with_context(|| format!("failed to open reconcile script for alias `{alias}`"))
    });

    match result {
        Ok(()) => {
            state.status_message = format!("Edited reconcile script for `{alias}`");
        }
        Err(err) => {
            state.status_message = err.to_string();
        }
    }
    Ok(())
}

fn resolve_alias_path(alias: &str) -> Option<PathBuf> {
    let cfg = crate::config_dir();
    [
        cfg.join("aliases").join(format!("{alias}.toml")),
        cfg.join(format!("{alias}.toml")),
        cfg.join(alias),
        cfg.join(format!("{alias}.conf")),
        cfg.join(format!("{alias}.rhai")),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn pause_terminal_for_subprocess<F>(run: F) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<()>,
{
    terminal::disable_raw_mode().context("failed to disable raw mode")?;
    execute!(io::stdout(), cursor::Show).context("failed to show cursor")?;
    let run_result = run();
    execute!(io::stdout(), cursor::Hide).context("failed to hide cursor")?;
    terminal::enable_raw_mode().context("failed to re-enable raw mode")?;
    run_result
}

fn render(
    state: &AppState,
    width: u16,
    height: u16,
    status_lines: &[String],
) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        terminal::Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;

    let title = format!(
        "chopper aliases  [{}]  tmux:{}",
        match state.layout {
            LayoutKind::Split => "split",
            LayoutKind::Modal => "modal",
        },
        state.tmux_mode.as_label()
    );
    writeln!(stdout, "{}", title.bold())?;

    let status_rows = status_lines.len() as u16;
    let content_rows = content_height(height, status_rows);
    if content_rows == 0 {
        for line in status_lines {
            writeln!(stdout, "{}", truncate_line(line, width as usize))?;
        }
        stdout.flush()?;
        return Ok(());
    }

    match state.layout {
        LayoutKind::Split => render_split_content(&mut stdout, state, width, content_rows)?,
        LayoutKind::Modal => render_modal_content(&mut stdout, state, width, content_rows)?,
    }

    let message_line = truncate_line(&format!("status: {}", state.status_message), width as usize);
    writeln!(stdout, "{}", message_line.dark_grey())?;
    for line in status_lines {
        writeln!(stdout, "{}", truncate_line(line, width as usize).reverse())?;
    }
    stdout.flush()?;
    Ok(())
}

fn render_split_content(
    stdout: &mut io::Stdout,
    state: &AppState,
    width: u16,
    rows: usize,
) -> anyhow::Result<()> {
    let left_width = split_left_width(width) as usize;
    let right_width = width.saturating_sub(left_width as u16).saturating_sub(1) as usize;
    let aliases_empty = state.aliases.is_empty();
    let selected_alias = state
        .aliases
        .get(state.selected)
        .map(String::as_str)
        .unwrap_or("<none>");

    for idx in 0..rows {
        let alias_row = state.scroll + idx;
        let left_line = if aliases_empty {
            match idx {
                0 => String::from("  aliases"),
                1 => String::from("  (empty)"),
                _ => String::new(),
            }
        } else if let Some(alias) = state.aliases.get(alias_row) {
            let selected = alias_row == state.selected;
            let pointer = if selected { ">" } else { " " };
            let focus = if selected && state.focus == PaneFocus::List {
                "*"
            } else {
                " "
            };
            format!("{pointer}{focus} {alias}")
        } else {
            String::new()
        };

        let right_line = split_right_line(idx, state, selected_alias, aliases_empty);

        let left_line = truncate_line(&left_line, left_width);
        let right_line = truncate_line(&right_line, right_width);
        writeln!(
            stdout,
            "{left:<left_width$}|{right}",
            left = left_line,
            right = right_line,
            left_width = left_width
        )?;
    }

    Ok(())
}

fn split_right_line(
    idx: usize,
    state: &AppState,
    selected_alias: &str,
    aliases_empty: bool,
) -> String {
    match idx {
        0 => format!(
            "{}{}",
            if state.focus == PaneFocus::Inspector {
                "* "
            } else {
                "  "
            },
            "Inspector"
        ),
        1 => format!("alias: {selected_alias}"),
        2 => resolve_alias_path(selected_alias)
            .map(|path| format!("path: {}", path.display()))
            .unwrap_or_else(|| String::from("path: <unresolved>")),
        3 if aliases_empty => String::from("No aliases configured."),
        // Keep both help lines visible when help is toggled, even in empty state.
        4 if state.show_help => {
            String::from("help: Enter edit-alias | e edit-reconcile | r refresh")
        }
        5 if state.show_help => String::from("help: arrows/jk move | h/l focus panes | q quit"),
        4 if aliases_empty => String::from("Add one: chopper --alias add <name> --exec <cmd>"),
        6 if aliases_empty && state.show_help => {
            String::from("Add one: chopper --alias add <name> --exec <cmd>")
        }
        _ => String::new(),
    }
}

fn render_modal_content(
    stdout: &mut io::Stdout,
    state: &AppState,
    width: u16,
    rows: usize,
) -> anyhow::Result<()> {
    if state.aliases.is_empty() {
        writeln!(stdout, "{}", "No aliases configured.".dark_grey())?;
        for _ in 1..rows {
            writeln!(stdout)?;
        }
        return Ok(());
    }

    let selected_alias = state
        .aliases
        .get(state.selected)
        .map(String::as_str)
        .unwrap_or("<none>");
    let info_line = format!("selected: {selected_alias}");
    writeln!(
        stdout,
        "{}",
        truncate_line(&info_line, width as usize).dark_grey()
    )?;

    let list_rows = rows.saturating_sub(1);
    for idx in 0..list_rows {
        let alias_row = state.scroll + idx;
        let line = if let Some(alias) = state.aliases.get(alias_row) {
            let pointer = if alias_row == state.selected {
                ">"
            } else {
                " "
            };
            format!("{pointer} {alias}")
        } else {
            String::new()
        };
        writeln!(stdout, "{}", truncate_line(&line, width as usize))?;
    }

    Ok(())
}

fn split_left_width(width: u16) -> u16 {
    let base = (width / 3).max(24);
    base.min(width.saturating_sub(30))
}

fn ensure_selection_visible(state: &mut AppState, list_height: usize) {
    if state.aliases.is_empty() {
        state.selected = 0;
        state.scroll = 0;
        return;
    }

    if state.selected >= state.aliases.len() {
        state.selected = state.aliases.len() - 1;
    }

    if list_height == 0 {
        state.scroll = state.selected;
        return;
    }

    if state.selected < state.scroll {
        state.scroll = state.selected;
    } else if state.selected >= state.scroll + list_height {
        state.scroll = state.selected + 1 - list_height;
    }
}

fn content_height(height: u16, status_rows: u16) -> usize {
    // 1 title row + 1 status message row + N status hint rows.
    height.saturating_sub(2 + status_rows) as usize
}

fn alias_viewport_rows(layout: LayoutKind, content_rows: usize) -> usize {
    match layout {
        LayoutKind::Split => content_rows,
        // Modal reserves one row for "selected: ..."
        LayoutKind::Modal => content_rows.saturating_sub(1),
    }
}

fn layout_for_size(width: u16, height: u16) -> LayoutKind {
    if width >= SPLIT_MIN_WIDTH && height >= SPLIT_MIN_HEIGHT {
        LayoutKind::Split
    } else {
        LayoutKind::Modal
    }
}

fn status_hint_lines(width: u16, layout: LayoutKind) -> Vec<String> {
    let mut primary = String::from(
        "j/k, ↑/↓ move   Enter edit alias   e edit reconcile   r refresh   ? help   q quit",
    );
    if layout == LayoutKind::Split {
        primary.push_str("   h/l or ←/→ focus");
    }
    if primary.chars().count() <= width as usize {
        return vec![primary];
    }

    vec![
        String::from("move: j/k or ↑/↓   alias edit: Enter   reconcile edit: e"),
        String::from("refresh: r   focus: h/l or ←/→   help: ?   quit: q"),
    ]
}

fn truncate_line(input: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut count = 0usize;
    for ch in input.chars() {
        if count + 1 >= width {
            break;
        }
        out.push(ch);
        count += 1;
    }
    if input.chars().count() <= width {
        input.to_string()
    } else if width <= 1 {
        String::new()
    } else {
        format!("{out}…")
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> anyhow::Result<Self> {
        terminal::enable_raw_mode().context("failed to enable raw mode")?;
        execute!(io::stdout(), cursor::Hide).context("failed to hide cursor")?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0),
            cursor::Show
        );
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        alias_viewport_rows, ensure_selection_visible, layout_for_size, split_right_line,
        status_hint_lines, AppState, LayoutKind, PaneFocus,
    };
    use crate::tui_nvim::TmuxMode;

    #[test]
    fn layout_prefers_split_on_large_terminals() {
        assert_eq!(layout_for_size(120, 30), LayoutKind::Split);
    }

    #[test]
    fn layout_falls_back_to_modal_on_small_terminals() {
        assert_eq!(layout_for_size(90, 30), LayoutKind::Modal);
        assert_eq!(layout_for_size(120, 20), LayoutKind::Modal);
    }

    #[test]
    fn status_bar_prefers_single_line_on_wide_terminal() {
        let lines = status_hint_lines(140, LayoutKind::Split);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn status_bar_uses_two_lines_when_space_is_tight() {
        let lines = status_hint_lines(50, LayoutKind::Split);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn modal_alias_rows_reserve_one_info_line() {
        assert_eq!(alias_viewport_rows(LayoutKind::Modal, 10), 9);
        assert_eq!(alias_viewport_rows(LayoutKind::Modal, 1), 0);
        assert_eq!(alias_viewport_rows(LayoutKind::Split, 10), 10);
    }

    #[test]
    fn ensure_selection_visible_uses_modal_alias_row_budget() {
        let mut state = AppState {
            aliases: (0..10).map(|idx| format!("alias-{idx}")).collect(),
            selected: 9,
            scroll: 0,
            focus: PaneFocus::List,
            layout: LayoutKind::Modal,
            show_help: false,
            status_message: String::new(),
            tmux_mode: TmuxMode::Off,
        };

        // Modal content rows might be 5, but one line is reserved for selected info.
        let alias_rows = alias_viewport_rows(LayoutKind::Modal, 5);
        ensure_selection_visible(&mut state, alias_rows);
        assert_eq!(alias_rows, 4);
        assert_eq!(state.scroll, 6);
    }

    #[test]
    fn split_right_line_keeps_both_help_lines_when_empty() {
        let state = AppState {
            aliases: Vec::new(),
            selected: 0,
            scroll: 0,
            focus: PaneFocus::List,
            layout: LayoutKind::Split,
            show_help: true,
            status_message: String::new(),
            tmux_mode: TmuxMode::Off,
        };

        let help_1 = split_right_line(4, &state, "<none>", true);
        let help_2 = split_right_line(5, &state, "<none>", true);
        let add_one = split_right_line(6, &state, "<none>", true);
        assert!(help_1.starts_with("help: Enter edit-alias"));
        assert!(help_2.starts_with("help: arrows/jk move"));
        assert!(add_one.starts_with("Add one: chopper --alias add"));
    }
}
