# chopper FAQ

## Where should I start?

Start at the root `README.md` for quickstart and common commands.

## I just need command snippets. Where?

Use [`cli-reference.md`](cli-reference.md) for a full command reference, or [`examples.md`](examples.md) for copy/paste snippets.

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
ASCII case-insensitive). Blank/unknown values (including CRLF-wrapped strings) keep
cache enabled. Falsey values (`0`, `false`, `no`, `off`) also keep cache
enabled. Non-ASCII lookalikes (for example `ＴＲＵＥ`) are treated as unknown.

## How do I disable reconcile for one invocation?

```bash
CHOPPER_DISABLE_RECONCILE=1 chopper <alias> [args...]
```

Uses the same truthy parsing as cache disable; blank/unknown values keep
reconcile enabled, and falsey values (`0`, `false`, `no`, `off`) keep
reconcile enabled too. Non-ASCII lookalikes are treated as unknown.

## Where are aliases loaded from?

Config root:

```text
${XDG_CONFIG_HOME:-~/.config}/chopper
```

Lookup order:

1. `<name>/exe.toml`
2. `aliases/<name>.toml` (legacy)
3. `<name>.toml` (legacy)

Legacy TOML configs automatically get canonical `<name>/exe.toml` symlinks on
lookup. The original files remain in place, preserving existing edit paths and
relative-path behavior.

---

## See also

- [`cli-reference.md`](cli-reference.md)
- [`troubleshooting.md`](troubleshooting.md)
- [`docs index`](README.md)
- [`root README`](../README.md)
