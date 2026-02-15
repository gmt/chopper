# chopper FAQ

## Where should I start?

Start at the root `README.md` for quickstart and common commands.

## I just need command snippets. Where?

Use `doc/quick-reference.md`.

## I want copy/paste alias examples. Where?

Use `doc/examples.md` and `doc/templates/`.

## Something is broken. Where do I debug first?

Use `doc/troubleshooting.md`.

## Where are the exact edge-case semantics documented?

Use `doc/operational-spec.md`.

## How do I disable cache for one invocation?

```bash
CHOPPER_DISABLE_CACHE=1 chopper <alias> [args...]
```

## How do I disable reconcile for one invocation?

```bash
CHOPPER_DISABLE_RECONCILE=1 chopper <alias> [args...]
```

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
