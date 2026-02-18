use anyhow::Context;
use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::{Frame, Terminal};
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const SPLIT_MAX_LEFT_WIDTH: u16 = 60;
const SPLIT_MIN_RIGHT_WIDTH: u16 = 30;
const SPLIT_MIN_HEIGHT: u16 = 3;

type AppTerminal = Terminal<CrosstermBackend<io::Stdout>>;

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
enum TabStripMode {
    Full,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LayoutPlan {
    kind: LayoutKind,
    left_width: u16,
    tab_mode: TabStripMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneFocus {
    List,
    Inspector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlSurface {
    Toml,
    Reconcile,
}

impl ControlSurface {
    fn label(self) -> &'static str {
        match self {
            Self::Toml => "toml",
            Self::Reconcile => "reconcile",
        }
    }

    fn all() -> [Self; 2] {
        [Self::Toml, Self::Reconcile]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopAction {
    Continue,
    Refresh,
    ActivateCurrentSurface,
    ActivateReconcileQuick,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorMode {
    Browse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptKind {
    NewAlias,
    RenameAlias,
    DuplicateAlias,
    DeleteAlias,
}

#[derive(Debug, Clone)]
struct PromptState {
    kind: PromptKind,
    input: String,
    keep_configs: bool,
}

#[derive(Debug, Default, Clone)]
struct AliasArtifacts {
    selected_alias: Option<String>,
    toml_path: Option<PathBuf>,
    reconcile_script_path: Option<PathBuf>,
}

#[derive(Debug)]
struct AppState {
    aliases: Vec<String>,
    selected: usize,
    scroll: usize,
    focus: PaneFocus,
    layout: LayoutKind,
    active_surface: ControlSurface,
    artifacts: AliasArtifacts,
    inspector_mode: InspectorMode,
    prompt: Option<PromptState>,
    alert_message: Option<String>,
    diagnostics: Vec<String>,
    tmux_mode: crate::tui_nvim::TmuxMode,
}

#[derive(Debug)]
struct InspectorDetailContent {
    lines: Vec<String>,
    cursor_line: Option<usize>,
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
        active_surface: ControlSurface::Toml,
        artifacts: AliasArtifacts::default(),
        inspector_mode: InspectorMode::Browse,
        prompt: None,
        alert_message: None,
        diagnostics: Vec::new(),
        tmux_mode: options.tmux_mode,
    };
    let mut diagnostics_rx = Some(spawn_config_scan_worker());

    let _guard = TerminalGuard::new()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).context("failed to initialize terminal backend")?;
    terminal.clear().context("failed to clear terminal")?;

    loop {
        receive_config_diagnostics(&mut state, &mut diagnostics_rx);
        let (width, height) = terminal::size().context("failed to read terminal size")?;
        sync_artifacts_for_selection(&mut state);
        let layout_plan = compute_layout(width, height, &state);
        state.layout = layout_plan.kind;
        let content_rows = content_height(
            height,
            state.alert_message.is_some() || state.prompt.is_some(),
            !state.diagnostics.is_empty(),
        );
        let alias_rows = alias_viewport_rows(layout_plan.kind, content_rows);
        ensure_selection_visible(&mut state, alias_rows);
        sync_artifacts_for_selection(&mut state);

        draw(&mut terminal, &state, layout_plan)?;

        if !event::poll(Duration::from_millis(100)).context("failed to poll keyboard events")? {
            continue;
        }
        let event = event::read().context("failed to read keyboard event")?;
        let action = handle_event(&mut state, event, alias_rows);
        match action {
            LoopAction::Continue => {}
            LoopAction::Refresh => {
                refresh_aliases(&mut state)?;
                diagnostics_rx = Some(spawn_config_scan_worker());
            }
            LoopAction::ActivateCurrentSurface => {
                activate_current_surface(&mut state, &mut terminal)?;
            }
            LoopAction::ActivateReconcileQuick => {
                activate_reconcile_quick(&mut state, &mut terminal)?;
            }
            LoopAction::Quit => break,
        }
    }

    Ok(())
}

fn spawn_config_scan_worker() -> Receiver<Vec<String>> {
    let (tx, rx) = mpsc::channel();
    let config_root = crate::config_dir();
    std::thread::spawn(move || {
        let warnings = crate::config_diagnostics::scan_extension_warnings(&config_root);
        let _ = tx.send(warnings);
    });
    rx
}

fn receive_config_diagnostics(state: &mut AppState, receiver: &mut Option<Receiver<Vec<String>>>) {
    let Some(rx) = receiver.take() else {
        return;
    };
    match rx.try_recv() {
        Ok(warnings) => {
            state.diagnostics = warnings;
        }
        Err(mpsc::TryRecvError::Empty) => {
            *receiver = Some(rx);
        }
        Err(mpsc::TryRecvError::Disconnected) => {}
    }
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
    if state.prompt.is_some() {
        return handle_prompt_key_event(state, key, list_height);
    }
    state.alert_message = None;

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => LoopAction::Quit,
        KeyCode::Up | KeyCode::Char('k') => {
            if state.selected > 0 {
                state.selected -= 1;
                ensure_selection_visible(state, list_height);
            }
            LoopAction::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.selected + 1 < state.aliases.len() {
                state.selected += 1;
                ensure_selection_visible(state, list_height);
            }
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
            state.inspector_mode = InspectorMode::Browse;
            LoopAction::Continue
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if state.focus == PaneFocus::List {
                state.focus = PaneFocus::Inspector;
                state.inspector_mode = InspectorMode::Browse;
            } else {
                cycle_surface(state, true);
            }
            LoopAction::Continue
        }
        KeyCode::Tab => {
            cycle_surface(state, true);
            LoopAction::Continue
        }
        KeyCode::BackTab => {
            cycle_surface(state, false);
            LoopAction::Continue
        }
        KeyCode::Char('1') => {
            set_active_surface(state, ControlSurface::Toml);
            LoopAction::Continue
        }
        KeyCode::Char('2') => {
            set_active_surface(state, ControlSurface::Reconcile);
            LoopAction::Continue
        }
        KeyCode::Char('+') => {
            state.prompt = Some(PromptState {
                kind: PromptKind::NewAlias,
                input: String::new(),
                keep_configs: false,
            });
            LoopAction::Continue
        }
        KeyCode::Char('%') => {
            let seed = state
                .aliases
                .get(state.selected)
                .cloned()
                .unwrap_or_default();
            state.prompt = Some(PromptState {
                kind: PromptKind::RenameAlias,
                input: seed,
                keep_configs: false,
            });
            LoopAction::Continue
        }
        KeyCode::Char('!') => {
            state.prompt = Some(PromptState {
                kind: PromptKind::DuplicateAlias,
                input: String::new(),
                keep_configs: false,
            });
            LoopAction::Continue
        }
        KeyCode::Char('-') => {
            state.prompt = Some(PromptState {
                kind: PromptKind::DeleteAlias,
                input: String::new(),
                keep_configs: false,
            });
            LoopAction::Continue
        }
        KeyCode::Char('r') => LoopAction::Refresh,
        KeyCode::Char('e') => LoopAction::ActivateReconcileQuick,
        KeyCode::Enter => {
            if state.focus == PaneFocus::List {
                state.focus = PaneFocus::Inspector;
                state.inspector_mode = InspectorMode::Browse;
                return LoopAction::Continue;
            }
            LoopAction::ActivateCurrentSurface
        }
        _ => LoopAction::Continue,
    }
}

fn handle_prompt_key_event(state: &mut AppState, key: KeyEvent, list_height: usize) -> LoopAction {
    let Some(prompt) = state.prompt.as_mut() else {
        return LoopAction::Continue;
    };
    match key.code {
        KeyCode::Esc => {
            state.prompt = None;
            LoopAction::Continue
        }
        KeyCode::Enter => {
            submit_prompt(state, list_height);
            LoopAction::Continue
        }
        KeyCode::Backspace => {
            prompt.input.pop();
            LoopAction::Continue
        }
        KeyCode::Char('k') | KeyCode::Char('K') => {
            if prompt.kind == PromptKind::DeleteAlias {
                prompt.keep_configs = !prompt.keep_configs;
                return LoopAction::Continue;
            }
            let ch = match key.code {
                KeyCode::Char(ch) => ch,
                _ => unreachable!(),
            };
            prompt.input.push(ch);
            LoopAction::Continue
        }
        KeyCode::Char(ch) => {
            prompt.input.push(ch);
            LoopAction::Continue
        }
        _ => LoopAction::Continue,
    }
}

fn submit_prompt(state: &mut AppState, list_height: usize) {
    let Some(prompt) = state.prompt.take() else {
        return;
    };
    let input = prompt.input.trim().to_string();
    let result = match prompt.kind {
        PromptKind::NewAlias => {
            if input.is_empty() {
                Err(anyhow::anyhow!("new alias name cannot be blank"))
            } else {
                crate::alias_admin::create_alias(&input).map(|_| {
                    refresh_aliases_and_select(state, &input, list_height);
                    state.active_surface = ControlSurface::Toml;
                    state.focus = PaneFocus::Inspector;
                    state.inspector_mode = InspectorMode::Browse;
                })
            }
        }
        PromptKind::RenameAlias => {
            let Some(source_alias) = state.aliases.get(state.selected).cloned() else {
                return;
            };
            if input.is_empty() {
                Err(anyhow::anyhow!("rename target alias cannot be blank"))
            } else {
                crate::alias_admin::rename_alias(&source_alias, &input).map(|_| {
                    refresh_aliases_and_select(state, &input, list_height);
                })
            }
        }
        PromptKind::DuplicateAlias => {
            let Some(source_alias) = state.aliases.get(state.selected).cloned() else {
                return;
            };
            if input.is_empty() {
                Err(anyhow::anyhow!("duplicate target alias cannot be blank"))
            } else {
                crate::alias_admin::duplicate_alias(&source_alias, &input).map(|_| {
                    refresh_aliases_and_select(state, &input, list_height);
                })
            }
        }
        PromptKind::DeleteAlias => {
            let Some(alias) = state.aliases.get(state.selected).cloned() else {
                return;
            };
            if matches!(input.as_str(), "y" | "Y" | "yes" | "YES" | "Yes") {
                crate::alias_admin::remove_alias_for_tui(&alias, prompt.keep_configs)
                    .map(|_| refresh_aliases_after_delete(state, list_height))
            } else {
                state.alert_message = Some(String::from("delete aborted"));
                Ok(())
            }
        }
    };

    if let Err(err) = result {
        state.alert_message = Some(err.to_string());
    }
}

fn refresh_aliases_and_select(state: &mut AppState, alias: &str, list_height: usize) {
    if let Err(err) = refresh_aliases(state) {
        state.alert_message = Some(err.to_string());
        return;
    }
    if let Some(idx) = state.aliases.iter().position(|value| value == alias) {
        state.selected = idx;
    }
    ensure_selection_visible(state, list_height);
    invalidate_artifacts(state);
}

fn refresh_aliases_after_delete(state: &mut AppState, list_height: usize) {
    if let Err(err) = refresh_aliases(state) {
        state.alert_message = Some(err.to_string());
        return;
    }
    ensure_selection_visible(state, list_height);
    invalidate_artifacts(state);
}

fn invalidate_artifacts(state: &mut AppState) {
    state.artifacts.selected_alias = None;
    sync_artifacts_for_selection(state);
}

fn default_reconcile_doc(alias: &str) -> crate::alias_doc::AliasReconcileDoc {
    crate::alias_doc::AliasReconcileDoc {
        script: format!("{alias}.reconcile.rhai"),
        function: Some(String::from("reconcile")),
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
    sync_artifacts_for_selection(state);
    Ok(())
}

fn activate_current_surface(
    state: &mut AppState,
    terminal: &mut AppTerminal,
) -> anyhow::Result<()> {
    let Some(alias) = state.aliases.get(state.selected).cloned() else {
        state.alert_message = Some(String::from("No alias selected"));
        return Ok(());
    };
    warn_missing_targets_for_active_alias(state, &alias);
    let surface = state.active_surface;
    execute_surface_action(state, terminal, &alias, surface)?;
    Ok(())
}

fn activate_reconcile_quick(
    state: &mut AppState,
    terminal: &mut AppTerminal,
) -> anyhow::Result<()> {
    let Some(alias) = state.aliases.get(state.selected).cloned() else {
        state.alert_message = Some(String::from("No alias selected"));
        return Ok(());
    };

    warn_missing_targets_for_active_alias(state, &alias);
    set_active_surface(state, ControlSurface::Reconcile);
    execute_surface_action(state, terminal, &alias, ControlSurface::Reconcile)?;
    Ok(())
}

fn warn_missing_targets_for_active_alias(state: &mut AppState, alias: &str) {
    let Some(config_path) = state.artifacts.toml_path.clone() else {
        return;
    };
    let Ok(manifest) = crate::parser::parse(&config_path) else {
        return;
    };
    let warnings = crate::config_diagnostics::manifest_missing_target_warnings(&manifest);
    if warnings.is_empty() {
        return;
    }
    let first = warnings.first().map(String::as_str).unwrap_or("");
    state.alert_message = Some(format!("warning for `{alias}`: {first}"));
}

fn execute_surface_action(
    state: &mut AppState,
    terminal: &mut AppTerminal,
    alias: &str,
    surface: ControlSurface,
) -> anyhow::Result<()> {
    sync_artifacts_for_selection(state);
    let result = match surface {
        ControlSurface::Toml => {
            let path = if let Some(path) = state.artifacts.toml_path.clone() {
                path
            } else {
                let (doc, path) = crate::alias_admin::load_or_seed_alias_doc(alias)?;
                if !path.is_file() {
                    crate::alias_admin::save_alias_doc_at(&path, &doc)?;
                }
                path
            };
            pause_terminal_for_subprocess(terminal, || {
                crate::tui_nvim::open_alias_editor(&path, state.tmux_mode)
                    .with_context(|| format!("failed to open TOML config for alias `{alias}`"))
            })
        }
        ControlSurface::Reconcile => {
            if let Some(path) = state.artifacts.reconcile_script_path.clone() {
                pause_terminal_for_subprocess(terminal, || {
                    crate::tui_nvim::open_rhai_editor_with_mode(
                        &path,
                        &crate::rhai_api_catalog::exported_api_names(),
                        state.tmux_mode,
                    )
                    .with_context(|| format!("failed to open reconcile script for alias `{alias}`"))
                })
            } else {
                create_missing_reconcile_artifact(state, terminal, alias)?;
                return Ok(());
            }
        }
    };

    if let Err(err) = result {
        state.alert_message = Some(err.to_string());
        return Ok(());
    }

    refresh_aliases(state)?;
    Ok(())
}

fn reconcile_draft_template(alias: &str) -> String {
    format!(
        "// CHOPPER_DRAFT: reconcile script draft for `{alias}`\n// CHOPPER_DRAFT: write and quit to save, or :q! to abort\nfn reconcile(ctx) {{\n    #{{}}\n}}\n\nfn complete(ctx) {{\n    []\n}}\n"
    )
}

fn resolve_script_path_from_doc_path(script: &str, doc_path: &std::path::Path) -> PathBuf {
    let script_path = PathBuf::from(script);
    if script_path.is_absolute() {
        script_path
    } else {
        doc_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(script_path)
    }
}

fn create_missing_reconcile_artifact(
    state: &mut AppState,
    terminal: &mut AppTerminal,
    alias: &str,
) -> anyhow::Result<()> {
    let (mut doc, doc_path) = crate::alias_admin::load_or_seed_alias_doc(alias)?;
    let had_reconcile = doc.reconcile.is_some();
    let reconcile = doc
        .reconcile
        .get_or_insert_with(|| default_reconcile_doc(alias));
    let script_path = resolve_script_path_from_doc_path(&reconcile.script, &doc_path);
    let template = reconcile_draft_template(alias);
    let persisted = pause_terminal_for_subprocess(terminal, || {
        crate::tui_nvim::open_rhai_draft_editor_with_mode(
            &script_path,
            &template,
            &crate::rhai_api_catalog::exported_api_names(),
            state.tmux_mode,
        )
        .with_context(|| format!("failed to open reconcile draft for alias `{alias}`"))
    })?;

    if persisted {
        if !had_reconcile {
            crate::alias_admin::save_alias_doc_at(&doc_path, &doc)?;
        }
        refresh_aliases(state)?;
        state.alert_message = Some(format!(
            "created reconcile script for `{alias}` at {}",
            script_path.display()
        ));
    } else {
        state.alert_message = Some(format!("reconcile creation aborted for alias `{alias}`"));
    }
    Ok(())
}

fn resolve_alias_path(alias: &str) -> Option<PathBuf> {
    let cfg = crate::config_dir();
    [
        cfg.join("aliases").join(format!("{alias}.toml")),
        cfg.join(format!("{alias}.toml")),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn resolve_toml_path(alias: &str) -> Option<PathBuf> {
    let cfg = crate::config_dir();
    [
        cfg.join("aliases").join(format!("{alias}.toml")),
        cfg.join(format!("{alias}.toml")),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn collect_alias_artifacts(alias: &str) -> AliasArtifacts {
    let resolved_config_path = resolve_alias_path(alias);
    let toml_path = resolve_toml_path(alias);
    let reconcile_script_path = resolved_config_path
        .as_ref()
        .and_then(|path| crate::parser::parse(path).ok())
        .and_then(|manifest| manifest.reconcile)
        .map(|reconcile| reconcile.script)
        .filter(|path| path.is_file());

    AliasArtifacts {
        selected_alias: Some(alias.to_string()),
        toml_path,
        reconcile_script_path,
    }
}

fn sync_artifacts_for_selection(state: &mut AppState) {
    let selected_alias = state.aliases.get(state.selected).cloned();
    if selected_alias == state.artifacts.selected_alias {
        return;
    }

    state.artifacts = selected_alias
        .as_deref()
        .map(collect_alias_artifacts)
        .unwrap_or_default();
}

fn surface_has_data(artifacts: &AliasArtifacts, surface: ControlSurface) -> bool {
    match surface {
        ControlSurface::Toml => true,
        ControlSurface::Reconcile => artifacts.reconcile_script_path.is_some(),
    }
}

fn set_active_surface(state: &mut AppState, surface: ControlSurface) {
    state.active_surface = surface;
    state.inspector_mode = InspectorMode::Browse;
    if state.layout == LayoutKind::Modal {
        state.focus = PaneFocus::Inspector;
    }
}

fn cycle_surface(state: &mut AppState, forward: bool) {
    let all = ControlSurface::all();
    let Some(mut idx) = all.iter().position(|value| *value == state.active_surface) else {
        state.active_surface = ControlSurface::Toml;
        state.inspector_mode = InspectorMode::Browse;
        return;
    };
    for _ in 0..all.len() {
        idx = if forward {
            (idx + 1) % all.len()
        } else {
            (idx + all.len() - 1) % all.len()
        };
        state.active_surface = all[idx];
        state.inspector_mode = InspectorMode::Browse;
        return;
    }
}

fn pause_terminal_for_subprocess<T, F>(terminal: &mut AppTerminal, run: F) -> anyhow::Result<T>
where
    F: FnOnce() -> anyhow::Result<T>,
{
    terminal::disable_raw_mode().context("failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)
        .context("failed to restore terminal before launching editor")?;
    let run_result = run();
    let restore_screen = execute!(terminal.backend_mut(), EnterAlternateScreen, cursor::Hide)
        .context("failed to re-enter alternate terminal screen");
    let restore_mode = terminal::enable_raw_mode().context("failed to re-enable raw mode");
    let clear_result = terminal
        .clear()
        .context("failed to clear terminal after returning from editor");

    let mut cleanup_errors = Vec::new();
    if let Err(err) = restore_screen {
        cleanup_errors.push(format!("{err:#}"));
    }
    if let Err(err) = restore_mode {
        cleanup_errors.push(format!("{err:#}"));
    }
    if let Err(err) = clear_result {
        cleanup_errors.push(format!("{err:#}"));
    }

    match (run_result, cleanup_errors.is_empty()) {
        (Ok(value), true) => Ok(value),
        (Ok(_), false) => {
            anyhow::bail!("terminal cleanup failed: {}", cleanup_errors.join("; "))
        }
        (Err(err), true) => Err(err),
        (Err(err), false) => {
            anyhow::bail!(
                "{err:#}; terminal cleanup also failed: {}",
                cleanup_errors.join("; ")
            )
        }
    }
}

fn draw(
    terminal: &mut AppTerminal,
    state: &AppState,
    layout_plan: LayoutPlan,
) -> anyhow::Result<()> {
    terminal
        .draw(|frame| render_frame(frame, state, layout_plan))
        .context("failed to render terminal frame")?;
    Ok(())
}

fn render_frame(frame: &mut Frame, state: &AppState, layout_plan: LayoutPlan) {
    let area = frame.area();
    if area.width == 0 || area.height == 0 {
        return;
    }

    let has_status_row = state.alert_message.is_some() || state.prompt.is_some();
    let has_diagnostics_row = !state.diagnostics.is_empty();
    let mut constraints = vec![Constraint::Length(1), Constraint::Min(0)];
    if has_diagnostics_row {
        constraints.push(Constraint::Length(1));
    }
    if has_status_row {
        constraints.push(Constraint::Length(1));
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    if chunks.len() < 2 {
        return;
    }

    render_banner(frame, chunks[0], state);
    render_content(frame, chunks[1], state, layout_plan);

    let mut next_row = 2usize;
    if has_diagnostics_row {
        if let Some(diagnostics_area) = chunks.get(next_row) {
            let summary = diagnostics_summary_line(state, diagnostics_area.width as usize);
            frame.render_widget(
                Paragraph::new(summary).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .bg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                ),
                *diagnostics_area,
            );
        }
        next_row += 1;
    }

    if let Some(status_area) = chunks.get(next_row) {
        if let Some(prompt) = &state.prompt {
            let prompt_text = prompt_hint(prompt, state);
            frame.render_widget(
                Paragraph::new(truncate_line(&prompt_text, status_area.width as usize)).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .bg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                ),
                *status_area,
            );
        } else if let Some(message) = &state.alert_message {
            frame.render_widget(
                Paragraph::new(truncate_line(message, status_area.width as usize)).style(
                    Style::default()
                        .fg(Color::Red)
                        .bg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                ),
                *status_area,
            );
        }
    }
}

fn diagnostics_summary_line(state: &AppState, width: usize) -> String {
    let total = state.diagnostics.len();
    let first = state
        .diagnostics
        .first()
        .map(String::as_str)
        .unwrap_or("configuration warning");
    if total <= 1 {
        truncate_line(&format!("Warnings: {first}"), width)
    } else {
        truncate_line(
            &format!("Warnings ({total}): {first} (+{} more)", total - 1),
            width,
        )
    }
}

fn prompt_hint(prompt: &PromptState, state: &AppState) -> String {
    match prompt.kind {
        PromptKind::NewAlias => format!(
            "New alias name: {} (Enter to create, Esc to cancel)",
            prompt.input
        ),
        PromptKind::RenameAlias => {
            let source = state
                .aliases
                .get(state.selected)
                .map(String::as_str)
                .unwrap_or("");
            format!(
                "Rename `{source}` to: {} (Enter to apply, Esc to cancel)",
                prompt.input
            )
        }
        PromptKind::DuplicateAlias => {
            let source = state
                .aliases
                .get(state.selected)
                .map(String::as_str)
                .unwrap_or("");
            format!(
                "Duplicate `{source}` as: {} (Enter to apply, Esc to cancel)",
                prompt.input
            )
        }
        PromptKind::DeleteAlias => {
            let source = state
                .aliases
                .get(state.selected)
                .map(String::as_str)
                .unwrap_or("");
            let keep_configs = if prompt.keep_configs { "[x]" } else { "[ ]" };
            format!(
                "Delete `{source}`? {keep_configs} keep configs (k toggles). type `y` + Enter (Esc cancels): {}",
                prompt.input,
            )
        }
    }
}

fn render_banner(frame: &mut Frame, area: Rect, state: &AppState) {
    let guidance = banner_guidance(state);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("chopper", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::raw(truncate_line(
                &guidance,
                area.width.saturating_sub(9) as usize,
            )),
        ])),
        area,
    );
}

fn banner_guidance(state: &AppState) -> String {
    format!(
        "Enter: inspect/activate {} | Tab: tabs | +/%/!/-: alias ops | e: reconcile | r: refresh | q: quit",
        state.active_surface.label()
    )
}

fn render_content(frame: &mut Frame, area: Rect, state: &AppState, layout_plan: LayoutPlan) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    match layout_plan.kind {
        LayoutKind::Split => render_split_content(frame, area, state, layout_plan),
        LayoutKind::Modal => render_modal_content(frame, area, state, layout_plan.tab_mode),
    }
}

fn render_split_content(frame: &mut Frame, area: Rect, state: &AppState, layout_plan: LayoutPlan) {
    if area.width < 3 {
        render_modal_content(frame, area, state, layout_plan.tab_mode);
        return;
    }

    let left_width = layout_plan
        .left_width
        .min(area.width.saturating_sub(2))
        .max(1);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_width),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);
    if columns.len() < 3 {
        return;
    }

    render_alias_list(frame, columns[0], state);

    let rows = area.height as usize;
    let separator = std::iter::repeat_n("|", rows)
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(Paragraph::new(separator), columns[1]);

    render_inspector(frame, columns[2], state, layout_plan.tab_mode);
}

fn render_alias_list(frame: &mut Frame, area: Rect, state: &AppState) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let rows = area.height as usize;
    let overflow = state.aliases.len() > rows;
    let (list_area, scrollbar_area) = if overflow && area.width > 1 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        (columns[0], Some(columns[1]))
    } else {
        (area, None)
    };

    let lines = split_left_lines(state, list_area.height as usize, list_area.width as usize);
    frame.render_widget(Paragraph::new(lines), list_area);

    if let Some(scrollbar_area) = scrollbar_area {
        let mut scrollbar_state = ScrollbarState::new(state.aliases.len()).position(state.scroll);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }
}

fn split_left_lines(state: &AppState, rows: usize, width: usize) -> Vec<Line<'static>> {
    (0..rows)
        .map(|idx| {
            let alias_row = state.scroll + idx;
            let line = if state.aliases.is_empty() {
                match idx {
                    0 => Line::from(truncate_line("aliases", width)),
                    1 => Line::from(truncate_line("(empty)", width)),
                    _ => Line::from(""),
                }
            } else if let Some(alias) = state.aliases.get(alias_row) {
                let selected = alias_row == state.selected;
                let truncated = truncate_line(alias, width);
                if selected {
                    let padded = pad_line_to_width(&truncated, width);
                    Line::from(Span::styled(padded, selected_alias_style(state)))
                } else {
                    Line::from(truncated)
                }
            } else {
                Line::from("")
            };
            line
        })
        .collect()
}

fn selected_alias_style(state: &AppState) -> Style {
    if state.layout == LayoutKind::Split && state.focus == PaneFocus::Inspector {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::REVERSED)
    } else {
        Style::default().add_modifier(Modifier::REVERSED)
    }
}

fn pad_line_to_width(input: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut line = truncate_line(input, width);
    let chars = line.chars().count();
    if chars >= width {
        return line;
    }
    line.push_str(&" ".repeat(width - chars));
    line
}

fn render_inspector(frame: &mut Frame, area: Rect, state: &AppState, tab_mode: TabStripMode) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);
    if chunks.len() < 3 {
        return;
    }

    let alias_line = state
        .artifacts
        .selected_alias
        .as_ref()
        .map(|name| format!("alias: {name}"))
        .unwrap_or_else(|| String::from("alias: <none>"));
    frame.render_widget(
        Paragraph::new(truncate_line(&alias_line, chunks[0].width as usize))
            .style(Style::default().fg(Color::DarkGray)),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(surface_tabs_line(state, tab_mode)),
        chunks[1],
    );

    render_inspector_details(frame, chunks[2], state);
}

fn surface_tabs_line(state: &AppState, tab_mode: TabStripMode) -> Line<'static> {
    if tab_mode == TabStripMode::Compact {
        return Line::from(Span::styled(
            state.active_surface.label(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let mut spans = Vec::new();
    for surface in ControlSurface::all() {
        let has_data = surface_has_data(&state.artifacts, surface);
        let active = state.active_surface == surface;
        let label = if active {
            format!("[{}]", surface.label())
        } else {
            format!(" {} ", surface.label())
        };
        let style = if active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if has_data {
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

fn render_inspector_details(frame: &mut Frame, area: Rect, state: &AppState) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let rows = area.height as usize;
    if rows == 0 {
        return;
    }

    let details = surface_detail_content(state);
    let total_lines = details.lines.len();
    let overflow = total_lines > rows;
    let (text_area, scrollbar_area) = if overflow && area.width > 1 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        (columns[0], Some(columns[1]))
    } else {
        (area, None)
    };

    let scroll = inspector_scroll_position(details.cursor_line, rows, total_lines);
    let visible_lines = details
        .lines
        .into_iter()
        .skip(scroll)
        .take(rows)
        .map(|line| truncate_line(&line, text_area.width as usize))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(visible_lines.join("\n")), text_area);

    if let Some(scrollbar_area) = scrollbar_area {
        let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }
}

fn surface_detail_content(state: &AppState) -> InspectorDetailContent {
    let mut lines = Vec::new();
    match state.active_surface {
        ControlSurface::Toml => {
            lines.push(String::from("toml editor"));
            if let Some(path) = &state.artifacts.toml_path {
                lines.push(format!("file: {}", path.display()));
                lines.push(String::from("Enter: open TOML editor"));
            } else {
                if let Some(alias) = state.artifacts.selected_alias.as_deref() {
                    lines.push(format!(
                        "file: {}",
                        crate::alias_admin::default_toml_path(alias).display()
                    ));
                } else {
                    lines.push(String::from("file: <select an alias>"));
                }
                lines.push(String::from("Enter: create and edit TOML config"));
            }
            lines.push(String::from("Tab: switch tabs | e: reconcile editor"));
        }
        ControlSurface::Reconcile => {
            if let Some(path) = &state.artifacts.reconcile_script_path {
                lines.push(String::from("reconcile tab"));
                lines.push(format!("script: {}", path.display()));
                lines.push(String::from("Enter/e: edit reconcile script"));
            } else {
                lines.push(String::from("reconcile tab (script missing)"));
                lines.push(String::from(
                    "Enter/e: open draft script; save persists script and TOML linkage",
                ));
            }
        }
    }
    InspectorDetailContent {
        lines,
        cursor_line: None,
    }
}

fn inspector_scroll_position(cursor_line: Option<usize>, rows: usize, total_lines: usize) -> usize {
    if rows == 0 || total_lines <= rows {
        return 0;
    }
    let max_scroll = total_lines - rows;
    let Some(cursor_line) = cursor_line else {
        return 0;
    };
    cursor_line
        .saturating_sub(rows.saturating_sub(1))
        .min(max_scroll)
}

fn render_modal_content(frame: &mut Frame, area: Rect, state: &AppState, tab_mode: TabStripMode) {
    if state.focus == PaneFocus::Inspector {
        render_inspector(frame, area, state, tab_mode);
        return;
    }

    if state.aliases.is_empty() {
        frame.render_widget(
            Paragraph::new("No aliases configured.").style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    // Hide tab strip when height-constrained to preserve vertical space for alias list
    if area.height < 3 {
        render_alias_list(frame, area, state);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    if chunks.len() < 2 {
        return;
    }

    frame.render_widget(
        Paragraph::new(surface_tabs_line(state, tab_mode)),
        chunks[0],
    );
    render_alias_list(frame, chunks[1], state);
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

fn content_height(height: u16, status_visible: bool, diagnostics_visible: bool) -> usize {
    // 1 banner row + optional diagnostics/status rows.
    height.saturating_sub(1 + u16::from(status_visible) + u16::from(diagnostics_visible)) as usize
}

fn alias_viewport_rows(layout: LayoutKind, content_rows: usize) -> usize {
    match layout {
        LayoutKind::Split => content_rows,
        // Modal reserves one row for the tab strip, unless height-constrained.
        LayoutKind::Modal => {
            if content_rows < 3 {
                content_rows
            } else {
                content_rows.saturating_sub(1)
            }
        }
    }
}

fn compute_layout(width: u16, height: u16, state: &AppState) -> LayoutPlan {
    let left_width = required_left_width(state);
    let separator_width = 1u16;
    let full_tabs_width = full_tab_strip_width();
    let compact_tabs_width = compact_tab_strip_width(state);

    let right_width = width
        .saturating_sub(left_width)
        .saturating_sub(separator_width);

    let has_enough_height = height >= SPLIT_MIN_HEIGHT;

    if left_width
        .saturating_add(separator_width)
        .saturating_add(full_tabs_width)
        <= width
        && right_width >= SPLIT_MIN_RIGHT_WIDTH
        && has_enough_height
    {
        LayoutPlan {
            kind: LayoutKind::Split,
            left_width,
            tab_mode: TabStripMode::Full,
        }
    } else if left_width
        .saturating_add(separator_width)
        .saturating_add(compact_tabs_width)
        <= width
        && right_width >= SPLIT_MIN_RIGHT_WIDTH
        && has_enough_height
    {
        LayoutPlan {
            kind: LayoutKind::Split,
            left_width,
            tab_mode: TabStripMode::Compact,
        }
    } else {
        LayoutPlan {
            kind: LayoutKind::Modal,
            left_width: 0,
            tab_mode: if width >= full_tabs_width {
                TabStripMode::Full
            } else {
                TabStripMode::Compact
            },
        }
    }
}

fn required_left_width(state: &AppState) -> u16 {
    let max_alias = state
        .aliases
        .iter()
        .map(|alias| alias.chars().count() as u16)
        .max()
        .unwrap_or(0);
    let min_width = "aliases".chars().count() as u16 + 2 + 1;
    max_alias
        .saturating_add(2)
        .saturating_add(1)
        .max(min_width)
        .min(SPLIT_MAX_LEFT_WIDTH)
}

fn full_tab_strip_width() -> u16 {
    ControlSurface::all()
        .into_iter()
        .map(|surface| surface.label().chars().count() as u16 + 3)
        .sum()
}

fn compact_tab_strip_width(state: &AppState) -> u16 {
    state.active_surface.label().chars().count() as u16
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
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)
            .context("failed to initialize terminal screen")?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
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
        alias_viewport_rows, compute_layout, content_height, cycle_surface,
        ensure_selection_visible, handle_key_event, inspector_scroll_position, set_active_surface,
        surface_has_data, AppState, ControlSurface, InspectorMode, LayoutKind, LoopAction,
        PaneFocus, TabStripMode,
    };
    use crate::tui_nvim::TmuxMode;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn sample_state(layout: LayoutKind) -> AppState {
        AppState {
            aliases: (0..10).map(|idx| format!("alias-{idx}")).collect(),
            selected: 0,
            scroll: 0,
            focus: PaneFocus::List,
            layout,
            active_surface: ControlSurface::Toml,
            artifacts: Default::default(),
            inspector_mode: InspectorMode::Browse,
            prompt: None,
            alert_message: None,
            diagnostics: Vec::new(),
            tmux_mode: TmuxMode::Off,
        }
    }

    #[test]
    fn content_driven_layout_uses_full_tabs_when_split_fits() {
        let state = sample_state(LayoutKind::Modal);
        let plan = compute_layout(60, 10, &state);
        assert_eq!(plan.kind, LayoutKind::Split);
        assert_eq!(plan.tab_mode, TabStripMode::Full);
    }

    #[test]
    fn content_driven_layout_compacts_tabs_before_modal_fallback() {
        let state = sample_state(LayoutKind::Modal);
        let plan = compute_layout(45, 10, &state);
        assert_eq!(plan.kind, LayoutKind::Split);
        assert_eq!(plan.tab_mode, TabStripMode::Full);
    }

    #[test]
    fn content_driven_layout_uses_modal_when_right_panel_too_narrow() {
        let state = sample_state(LayoutKind::Modal);
        // Width 30 would fit columns but right panel < SPLIT_MIN_RIGHT_WIDTH
        let plan = compute_layout(30, 10, &state);
        assert_eq!(plan.kind, LayoutKind::Modal);
    }

    #[test]
    fn content_driven_layout_uses_modal_when_terminal_too_short() {
        let state = sample_state(LayoutKind::Modal);
        // Width is fine but height < SPLIT_MIN_HEIGHT
        let plan = compute_layout(60, 2, &state);
        assert_eq!(plan.kind, LayoutKind::Modal);
    }

    #[test]
    fn content_driven_layout_falls_back_to_modal_for_wide_aliases() {
        let mut state = sample_state(LayoutKind::Modal);
        state.aliases = vec![String::from(
            "a-very-long-alias-name-that-forces-layout-to-fallback",
        )];
        let plan = compute_layout(40, 10, &state);
        assert_eq!(plan.kind, LayoutKind::Modal);
    }

    #[test]
    fn content_driven_layout_caps_left_width_for_long_aliases() {
        let mut state = sample_state(LayoutKind::Modal);
        state.aliases = vec!["x".repeat(200)];
        let plan = compute_layout(200, 20, &state);
        assert_eq!(plan.kind, LayoutKind::Split);
        assert_eq!(plan.left_width, 60);
    }

    #[test]
    fn content_driven_modal_uses_compact_tabs_on_narrow_terminals() {
        let state = sample_state(LayoutKind::Modal);
        let plan = compute_layout(16, 10, &state);
        assert_eq!(plan.kind, LayoutKind::Modal);
        assert_eq!(plan.tab_mode, TabStripMode::Compact);
    }

    #[test]
    fn content_height_accounts_for_optional_status_and_diagnostics_rows() {
        assert_eq!(content_height(30, false, false), 29);
        assert_eq!(content_height(30, true, false), 28);
        assert_eq!(content_height(30, false, true), 28);
        assert_eq!(content_height(30, true, true), 27);
    }

    #[test]
    fn modal_alias_rows_reserve_header_and_tabs_rows() {
        // Normal case: tab strip shown, 1 row reserved
        assert_eq!(alias_viewport_rows(LayoutKind::Modal, 10), 9);
        // Height-constrained: tab strip hidden, no rows reserved
        assert_eq!(alias_viewport_rows(LayoutKind::Modal, 2), 2);
        assert_eq!(alias_viewport_rows(LayoutKind::Modal, 1), 1);
        // Split always uses all rows
        assert_eq!(alias_viewport_rows(LayoutKind::Split, 10), 10);
    }

    #[test]
    fn inspector_scroll_tracks_cursor_visibility_when_overflowed() {
        assert_eq!(inspector_scroll_position(Some(0), 5, 20), 0);
        assert_eq!(inspector_scroll_position(Some(4), 5, 20), 0);
        assert_eq!(inspector_scroll_position(Some(5), 5, 20), 1);
        assert_eq!(inspector_scroll_position(Some(19), 5, 20), 15);
    }

    #[test]
    fn inspector_scroll_defaults_to_top_without_cursor_or_overflow() {
        assert_eq!(inspector_scroll_position(None, 5, 20), 0);
        assert_eq!(inspector_scroll_position(Some(8), 8, 8), 0);
        assert_eq!(inspector_scroll_position(Some(8), 0, 30), 0);
    }

    #[test]
    fn ensure_selection_visible_uses_modal_alias_row_budget() {
        let mut state = sample_state(LayoutKind::Modal);
        state.selected = 9;

        // Modal content rows might be 5, but one line is reserved for tab chrome.
        state.aliases = (0..20).map(|idx| format!("alias-{idx}")).collect();
        state.selected = 19;
        let alias_rows = alias_viewport_rows(LayoutKind::Modal, 5);
        ensure_selection_visible(&mut state, alias_rows);
        assert_eq!(alias_rows, 4);
        assert_eq!(state.scroll, 16);
    }

    #[test]
    fn control_surfaces_report_data_presence() {
        let artifacts = super::AliasArtifacts::default();
        assert!(surface_has_data(&artifacts, ControlSurface::Toml));
        assert!(!surface_has_data(&artifacts, ControlSurface::Reconcile));
    }

    #[test]
    fn release_key_events_are_ignored() {
        let mut state = sample_state(LayoutKind::Split);
        let key = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        let action = handle_key_event(&mut state, key, 10);
        assert_eq!(action, LoopAction::Continue);
    }

    #[test]
    fn right_focus_moves_into_inspector_in_any_layout() {
        let mut split = sample_state(LayoutKind::Split);
        let mut modal = sample_state(LayoutKind::Modal);
        let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);

        let split_action = handle_key_event(&mut split, key, 10);
        assert_eq!(split_action, LoopAction::Continue);
        assert_eq!(split.focus, PaneFocus::Inspector);

        let modal_action = handle_key_event(&mut modal, key, 10);
        assert_eq!(modal_action, LoopAction::Continue);
        assert_eq!(modal.focus, PaneFocus::Inspector);
    }

    #[test]
    fn selecting_missing_surface_is_allowed() {
        let mut state = sample_state(LayoutKind::Split);
        set_active_surface(&mut state, ControlSurface::Reconcile);
        assert_eq!(state.active_surface, ControlSurface::Reconcile);
    }

    #[test]
    fn surface_cycle_visits_all_surfaces_even_without_data() {
        let mut state = sample_state(LayoutKind::Split);
        cycle_surface(&mut state, true);
        assert_eq!(state.active_surface, ControlSurface::Reconcile);
        cycle_surface(&mut state, true);
        assert_eq!(state.active_surface, ControlSurface::Toml);
    }

    #[test]
    fn enter_on_toml_tab_moves_focus_into_inspector() {
        let mut state = sample_state(LayoutKind::Split);
        state.active_surface = ControlSurface::Toml;
        let action = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            10,
        );
        assert_eq!(action, LoopAction::Continue);
        assert_eq!(state.focus, PaneFocus::Inspector);
        assert_eq!(state.inspector_mode, InspectorMode::Browse);
    }

    #[test]
    fn enter_from_list_on_reconcile_moves_focus_without_activation() {
        let mut state = sample_state(LayoutKind::Split);
        state.active_surface = ControlSurface::Reconcile;
        state.focus = PaneFocus::List;
        let action = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            10,
        );
        assert_eq!(action, LoopAction::Continue);
        assert_eq!(state.focus, PaneFocus::Inspector);
        assert_eq!(state.inspector_mode, InspectorMode::Browse);
    }

    #[test]
    fn enter_from_inspector_on_toml_activates_editor_surface() {
        let mut state = sample_state(LayoutKind::Split);
        state.focus = PaneFocus::Inspector;
        state.active_surface = ControlSurface::Toml;
        state.inspector_mode = InspectorMode::Browse;
        let action = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            10,
        );
        assert_eq!(action, LoopAction::ActivateCurrentSurface);
    }
}
