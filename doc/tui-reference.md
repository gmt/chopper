# TUI reference

The TUI is launched with:

```bash
chopper --tui
```

It requires an interactive terminal (TTY).

---

## Main actions

- `l` list aliases
- `g` get alias details
- `a` add alias
- `s` set alias
- `r` remove alias (clean/dirty)
- `e` open Rhai script in `(n)vim`
- `q` quit

---

## Rhai editor integration

When choosing `e`:

- chopper writes a Rhai API completion dictionary to cache.
- if `nvim` is available:
  - launches with bootstrap config including keyword completion dictionary
  - best-effort `nvim-treesitter` setup via `pcall(...)`
- else if `vim` is available:
  - launches with dictionary-based completion enabled
- else:
  - returns an error indicating no editor was found

---

## Notes

- The TUI delegates alias actions to the same backend used by
  `chopper --alias ...`.
- Dirty remove deletes symlink only; clean remove deletes config + cache and
  best-effort symlink cleanup.
