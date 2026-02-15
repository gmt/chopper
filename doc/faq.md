# chopper FAQ

## Where should I start?

Start at the root `README.md` for quickstart and common commands.

## I just need command snippets. Where?

Use [`quick-reference.md`](quick-reference.md).

## I want copy/paste alias examples. Where?

Use [`examples.md`](examples.md) and [`templates/`](templates).

## Something is broken. Where do I debug first?

Use [`troubleshooting.md`](troubleshooting.md).

## Where are the exact edge-case semantics documented?

Use [`operational-spec.md`](operational-spec.md).

## How do I disable cache for one invocation?

```bash
CHOPPER_DISABLE_CACHE=1 chopper <alias> [args...]
```

Only truthy values disable it (`1`, `true`, `yes`, `on`; trimmed and
case-insensitive). Blank/unknown values (including CRLF-wrapped strings) keep
cache enabled.

## How do I disable reconcile for one invocation?

```bash
CHOPPER_DISABLE_RECONCILE=1 chopper <alias> [args...]
```

Uses the same truthy parsing as cache disable; blank/unknown values keep
reconcile enabled.

## Where are aliases loaded from?

Config root:

```text
${XDG_CONFIG_HOME:-~/.config}/chopper
```

Lookup order:

1. `aliases/<name>.toml`
2. `<name>.toml`
3. `<name>`
4. `<name>.conf`
5. `<name>.rhai`

---

## See also

- [`quick-reference.md`](quick-reference.md)
- [`troubleshooting.md`](troubleshooting.md)
- [`docs index`](README.md)
- [`root README`](../README.md)
