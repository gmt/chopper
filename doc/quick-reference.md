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

Truthy values (case-insensitive, trimmed):

- `1`
- `true`
- `yes`
- `on`

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

- `README.md` → concise overview + quickstart
- `doc/examples.md` → copy/paste workflows
- `doc/troubleshooting.md` → diagnosis checklists
- `doc/operational-spec.md` → complete semantics and edge cases
