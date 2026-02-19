# chopper glossary

Quick definitions for recurring terms in the documentation.

## Alias

A named launch target resolved by `chopper`, usually backed by a TOML file
under `aliases/<name>.toml`.

## Direct mode

Invocation form where executable name is `chopper` (or recognized wrapper
variants like `chopper.exe`) and alias name is provided as first positional arg.

## Symlink mode

Invocation form where executable basename itself is treated as alias name.

## Manifest

Normalized in-memory representation of an alias after parse + validation.

## Legacy alias (removed)

Historical one-line alias files were removed; aliases are now TOML-only.

## Reconcile

Optional runtime patch phase powered by Rhai. Can modify args/env at launch.

## Patch map

Rhai return object containing supported keys such as `append_args`,
`replace_args`, `set_env`, and `remove_env`.

## Journal routing

Optional stderr forwarding behavior through `systemd-cat --namespace=...`
configured via `[journal]`.

## Journal broker preflight

Optional preflight call enabled by `journal.ensure = true`. `chopper` invokes
`${CHOPPER_JOURNAL_BROKER_CMD:-chopper-journal-broker} ensure --namespace <effective_namespace>`
before starting `systemd-cat`.

## User-scoped journal namespace

When `journal.user_scope = true`, `chopper` transforms logical
`journal.namespace` into `u<uid>-<sanitized-username>-<sanitized-namespace>`.

## Config root

Base directory for alias discovery:
`${XDG_CONFIG_HOME:-~/.config}/chopper` unless `CHOPPER_CONFIG_DIR` override is
provided.

## Cache root

Base directory for manifest cache:
`${XDG_CACHE_HOME:-~/.cache}/chopper` unless `CHOPPER_CACHE_DIR` override is
provided.

## Truthy env flag

Normalized disable flag value considered enabled:
`1`, `true`, `yes`, `on` (trimmed and ASCII case-insensitive).

---

## See also

- [`config-reference.md`](config-reference.md)
- [`cli-reference.md`](cli-reference.md)
- [`docs index`](README.md)
- [`root README`](../README.md)
