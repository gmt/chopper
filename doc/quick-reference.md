# chopper quick reference

Fast command lookup for day-to-day use.

---

## Core commands

```bash
chopper <alias> [args...]
chopper <alias> -- [args...]
```

Symlink mode:

```bash
ln -s /path/to/chopper /usr/local/bin/myalias
myalias [args...]
```

---

## Built-ins (direct mode)

```bash
chopper --help
chopper --version
chopper --print-config-dir
chopper --print-cache-dir
```

---

## Common environment overrides

```bash
CHOPPER_CONFIG_DIR=/path/to/config-root chopper <alias>
CHOPPER_CACHE_DIR=/path/to/cache-root chopper <alias>
```

---

## One-off debugging toggles

```bash
CHOPPER_DISABLE_CACHE=1 chopper <alias> [args...]
CHOPPER_DISABLE_RECONCILE=1 chopper <alias> [args...]
```

Truthy values (ASCII case-insensitive, trimmed):

- `1`
- `true`
- `yes`
- `on`

Matching is ASCII case-insensitive; non-ASCII lookalike tokens (for example
`ＴＲＵＥ`) are treated as unknown values.

Blank/unknown values (including whitespace or CRLF-wrapped values) keep cache
and reconcile enabled.
This includes tab-only blanks (for example `"\t\t"`) and unicode lookalike
tokens such as `Ｔrue`, plus CRLF + NBSP-wrapped falsey tokens such as
`"\r\n\u00A0FaLsE\u00A0\r\n"`.

Common falsey values that also keep features enabled:

- `0`
- `false`
- `no`
- `off`

---

## Alias file locations

Config root:

```text
${XDG_CONFIG_HOME:-~/.config}/chopper
```

Lookup order for alias `<name>`:

1. `aliases/<name>.toml`
2. `<name>.toml`
3. `<name>`
4. `<name>.conf`
5. `<name>.rhai`

---

## Where to read more

- [`start-here.md`](start-here.md) → role/task-based entry point
- [`../README.md`](../README.md) → concise overview + quickstart
- [`templates/`](templates) → starter alias/reconcile files
- [`examples.md`](examples.md) → copy/paste workflows
- [`troubleshooting.md`](troubleshooting.md) → diagnosis checklists
- [`operational-spec.md`](operational-spec.md) → complete semantics and edge cases
