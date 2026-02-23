# chopper glossary

Quick definitions for recurring terms in the documentation.

## Alias

A named launch target resolved by `chopper`, usually backed by a TOML file
under `aliases/<name>.toml`.

## Direct mode

Invocation form where executable name is `chopper` and alias name is provided
as first positional arg.

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

Optional preflight call enabled by `journal.ensure = true`. `chopper` calls the
`chopper-journal-broker` D-Bus service
(`com.chopperproject.JournalBroker1.EnsureNamespace`) to create/start the
journald namespace before starting `systemd-cat`. The broker validates UID
ownership, writes drop-in configs, and starts namespace sockets.

## User-scoped journal namespace

When `journal.user_scope = true` (the default), `chopper` transforms logical
`journal.namespace` into `u<uid>-<sanitized-username>-<sanitized-namespace>`.
Set `user_scope = false` for literal namespace passthrough.

## Journal policy options

Optional per-alias journal policy fields (`max_use`, `rate_limit_interval_usec`,
`rate_limit_burst`) passed to the broker via D-Bus. The broker enforces hard
server-side limits.

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
