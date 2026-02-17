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
- `Enter`: edit selected alias file in editor
- `e`: edit selected alias reconcile script in editor (if configured)
- `r`: refresh alias list
- `?`: toggle help text
- `q` or `Esc`: quit
- `h`/`l` or `Left`/`Right`: pane focus (split layout)

---

## Layout behavior

The TUI chooses between two layouts based on terminal size:

- **Split layout** on larger terminals:
  - alias list on the left
  - inspector/details on the right
  - compact command hints in the status bar
- **Modal layout** on smaller terminals:
  - single-pane list-centric view
  - same key bindings and status bar commands

Status bar hints prefer one line. If width is too tight, hints expand to two
compact lines.

---

## Editor integration and tmux behavior

Editing actions:

- `Enter`: edit selected alias config path
- `e`: edit selected alias reconcile script path (when `reconcile.script` is configured)

Both use `nvim` (preferred) or `vim` (fallback).

`--tmux` policy:

- `auto` (default):
  - if inside tmux, open editor in a right-side split pane
  - if not inside tmux and no tmux server is running, open editor in a fresh tmux session
  - if not inside tmux and a tmux server is already running, avoid creating a second session and use direct editor launch
- `on`:
  - require tmux; split inside tmux or open a dedicated tmux session outside tmux
- `off` / `--no-tmux`:
  - always use direct editor launch (tmuxless)

If neither `nvim` nor `vim` exists in `PATH`, TUI editing returns an error.

---

## Notes

- Alias discovery uses the same alias lookup/config roots as the rest of
  chopper.
- TUI editing resolves alias files using the same lookup order as runtime
  invocation (`aliases/<name>.toml`, `<name>.toml`, legacy files).
