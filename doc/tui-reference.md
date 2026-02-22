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
- `Enter`:
  - from list focus: move into inspector/wizard
  - from inspector focus on TOML rows:
    - normal row: edit/toggle that field
    - method row (`reconcile.function`, `bashcomp.rhai_function`): open editor at handler
- `Space`: open method chooser for the selected method row
- `e`: quick action to open reconcile handler in shared Rhai file
- `r`: refresh alias list
- `q` or `Esc`: quit
- `h`/`l` or `Left`/`Right`: list/inspector focus
  - split layout: move focus between panes
  - modal layout: switch between alias list view and inspector wizard view
- `+`: create alias (prompt)
- `%`: rename selected alias (prompt)
- `!`: duplicate selected alias (prompt)
- `-`: delete selected alias (confirmation prompt)
  - in the delete prompt, press `k` to toggle `keep configs?`

---

## Layout behavior

The TUI chooses layout from what can be shown without unreasonable truncation:

- **Split layout (preferred)**:
  - alias list on the left
  - single inspector/editor surface on the right (no tabs)
- **Modal layout (fallback)**:
  - single-pane alias list
  - inspector/editor opens as a wizard-like full-screen modal panel when focused
  - used only when split cannot remain functional

The top banner keeps `chopper` as the bold brand token and shows concise action
guidance (`Enter`, `Space`, `+/%/!/-`, `e`, `r`, `q`). Bottom rows are used for:

- asynchronous config warnings (extension-scan results), and
- prompts or temporary blocking/error messages.

When alias or inspector detail content exceeds visible rows, a vertical
scrollbar indicates overflow.

---

## Editor integration and tmux behavior

Editing actions:

- TOML rows are the primary control surface.
- Method rows:
  - `reconcile.function` and `bashcomp.rhai_function` wire handlers
  - blank method value disables that feature
  - `Space` opens a method chooser from compatible `fn <name>(ctx)` handlers
  - selecting a method rewires TOML and opens editor at that handler
  - `Enter` opens editor at current/default handler, seeding one if missing
- Shared Rhai file path is deterministic per alias: `<alias>.rhai` beside `<alias>.toml`.
- On editor close, TUI re-scans handlers and auto-syncs wiring:
  - removed configured method -> field is cleared (auto-unwire)
  - single-method rename detection rewires automatically

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
  invocation (`aliases/<name>.toml`, `<name>.toml`).
- Legacy `reconcile.script` / `bashcomp.rhai_script` values may still appear in old files, but wiring is method-first.
- Rendering is handled by ratatui on top of crossterm, using an alternate
  screen for interactive drawing.
- Editing actions temporarily leave the alternate screen/raw-mode session and
  restore it after the editor exits.
