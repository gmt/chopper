# TUI reference

The TUI is launched with:

```bash
chopper --tui
```

Optional tmux policy flags:

```bash
chopper --tui --tmux=auto
chopper --tui --tmux=on
chopper --tui --tmux=off
chopper --tui --no-tmux
```

It requires an interactive terminal (TTY).

---

## Alias-first navigation

The TUI opens directly into an alias list, not a command menu.

- `j`/`k` or `Up`/`Down`: move selection (or field cursor in TOML inspector menu)
- `g` / `Home`: jump to top
- `G` / `End`: jump to bottom
- `Tab` / `Shift+Tab`: cycle inspector tabs
- `1`..`4`: jump directly to `summary`, `toml`, `legacy`, `reconcile` tabs
- `Enter`: activate the active surface
  - `toml`: open/edit selected schema field inside TUI (no `(n)vim` handoff)
  - `legacy` / `reconcile`: open external editor for file content flows
- `e`: quick action for reconcile editing/creation
- `r`: refresh alias list
- `q` or `Esc`: quit
- `h`/`l` or `Left`/`Right`: list/inspector focus
  - split layout: move focus between panes
  - modal layout: switch between alias list view and inspector wizard view
- `+`: create alias (prompt)
- `%`: rename selected alias (prompt)
- `!`: duplicate selected alias (prompt)
- `-`: delete selected alias (confirmation prompt)

---

## Layout behavior

The TUI chooses layout from what can be shown without unreasonable truncation:

- **Split layout (preferred)**:
  - alias list on the left
  - inspector/editor on the right with tabs (`summary`, `toml`, `legacy`, `reconcile`)
  - if horizontal space tightens, tab chrome compacts to the active tab label
- **Modal layout (fallback)**:
  - single-pane alias list with a tab strip row above it
  - inspector/editor opens as a wizard-like full-screen modal panel when focused
  - used only when split cannot remain functional after compaction

The top banner keeps `chopper` as the bold brand token and shows concise action
guidance (`Enter`, `Tab`, `+/%/!/-`, `e`, `r`, `q`). A bottom status bar is
used for prompts and temporary blocking/error messages.

When aliases exceed visible rows, a vertical scrollbar indicates overflow.

---

## Editor integration and tmux behavior

Editing actions:

- `toml` tab:
  - schema-bound properties are edited inside the TUI inspector menu
  - optional sections (`journal`, `reconcile`, `bashcomp`) can be added/removed
    by toggling their enabled fields
  - no external `(n)vim` launch for normal TOML property updates
- `legacy` / `reconcile` tabs:
  - `Enter` opens external editor for unstructured file content
  - if missing, a draft is opened with instructional comment lines
  - saving persists the artifact; aborting with `:q!` discards the draft

External edit actions use `nvim` (preferred) or `vim` (fallback).

`--tmux` policy:

- `auto` (default):
  - if inside tmux, open editor directly in the current pane (no split pane)
  - if not inside tmux and no tmux server is running, open editor in a fresh tmux session
  - if not inside tmux and a tmux server is already running, avoid creating a second session and use direct editor launch
- `on`:
  - require tmux; use direct launch inside tmux or a dedicated tmux session outside tmux
- `off` / `--no-tmux`:
  - always use direct editor launch (tmuxless)

If neither `nvim` nor `vim` exists in `PATH`, TUI editing returns an error.

---

## Notes

- Alias discovery uses the same alias lookup/config roots as the rest of
  chopper.
- TUI editing resolves alias files using the same lookup order as runtime
  invocation (`aliases/<name>.toml`, `<name>.toml`, legacy files).
- Tabs are always selectable; visual emphasis indicates data presence.
- Rendering is handled by ratatui on top of crossterm, using an alternate
  screen for interactive drawing.
- Editing actions temporarily leave the alternate screen/raw-mode session and
  restore it after the editor exits.
