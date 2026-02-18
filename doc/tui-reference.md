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

- `j`/`k` or `Up`/`Down`: move selection
- `g` / `Home`: jump to top
- `G` / `End`: jump to bottom
- `Tab` / `Shift+Tab`: cycle inspector tabs
- `1`..`4`: jump directly to `summary`, `toml`, `legacy`, `reconcile` tabs
- `Enter`: run the active tab action for the selected alias
- `e`: quick action for reconcile editing (when an extant reconcile script exists)
- `r`: refresh alias list
- `q` or `Esc`: quit
- `h`/`l` or `Left`/`Right`: list/inspector focus (split layout); also tab cycling when already in inspector

---

## Layout behavior

The TUI chooses layout from what can be shown without unreasonable truncation:

- **Split layout (preferred)**:
  - alias list on the left
  - inspector on the right with tabs (`summary`, `toml`, `legacy`, `reconcile`)
  - if horizontal space tightens, tab chrome compacts to the active tab label
- **Modal layout (fallback)**:
  - single-pane alias list with a tab strip row above it
  - used only when split cannot remain functional after compaction

The top banner keeps `chopper` as the bold brand token and shows concise action
guidance (`Enter`, `Tab`, `e`, `r`, `q`). A bottom alert bar appears only for
temporary blocking/error messages.

When aliases exceed visible rows, a vertical scrollbar indicates overflow.

---

## Editor integration and tmux behavior

Editing actions:

- `Enter`: edits the file/action represented by the active tab
- `e`: fast path for reconcile script editing when the script exists

Both use `nvim` (preferred) or `vim` (fallback).

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
- Rendering is handled by ratatui on top of crossterm, using an alternate
  screen for interactive drawing.
- Editing actions temporarily leave the alternate screen/raw-mode session and
  restore it after the editor exits.
