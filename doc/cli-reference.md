# chopper CLI reference

Command-oriented reference for invocation and environment controls.

---

## Invocation forms

Direct mode:

```bash
chopper <alias> [args...]
chopper <alias> -- [args...]
```

Symlink mode:

```bash
ln -s /path/to/chopper /usr/local/bin/myalias
myalias [args...]
```

In symlink mode, executable basename is used as alias name and built-ins are
not treated specially.

---

## Built-ins (direct mode only)

```bash
chopper
chopper --help
chopper --version
chopper --print-config-dir
chopper --print-cache-dir
chopper --bashcomp
chopper --list-aliases
chopper --print-exec <alias>
chopper --print-bashcomp-mode <alias>
```

### Bash completion

```bash
chopper --bashcomp
```

Emits a sourceable bash completion script to stdout. Source it in your
`.bashrc` or save it persistently:

```bash
source <(chopper --bashcomp)
# or:
chopper --bashcomp > ~/.local/share/bash-completion/completions/chopper
```

### Introspection

```bash
chopper --list-aliases
```

Lists all configured alias names (one per line). Scans both the `aliases/`
subdirectory and the config root.

```bash
chopper --print-exec <alias>
```

Prints the resolved `exec` path for the given alias. Exits 0 on success,
1 if the alias cannot be resolved.

```bash
chopper --print-bashcomp-mode <alias>
```

Prints the bash completion mode for the alias: `disabled`, `passthrough`,
`custom`, or `normal`. Exits 0 always.

---

## Environment controls

Config/cache roots:

```bash
CHOPPER_CONFIG_DIR=/path/to/config-root chopper <alias> [args...]
CHOPPER_CACHE_DIR=/path/to/cache-root chopper <alias> [args...]
```

Feature toggles (per invocation):

```bash
CHOPPER_DISABLE_CACHE=<truthy> chopper <alias> [args...]
CHOPPER_DISABLE_RECONCILE=<truthy> chopper <alias> [args...]
```

Truthy values (trimmed, ASCII case-insensitive):

- `1`
- `true`
- `yes`
- `on`

Falsey/blank/unknown values (including whitespace/CRLF-wrapped values) leave
the feature enabled.

Common falsey values:

- `0`
- `false`
- `no`
- `off`

---

## Useful diagnostics

```bash
chopper --print-config-dir
chopper --print-cache-dir
chopper --print-exec <alias>
chopper --print-bashcomp-mode <alias>
chopper --list-aliases
CHOPPER_DISABLE_CACHE=1 chopper <alias> [args...]
CHOPPER_DISABLE_RECONCILE=1 chopper <alias> [args...]
```

---

## Related docs

- [`quick-reference.md`](quick-reference.md) for rapid lookup
- [`operational-spec.md`](operational-spec.md) for full semantics
- [`troubleshooting.md`](troubleshooting.md) for failure diagnosis
