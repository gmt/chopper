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

## Legacy alias

One-line alias file (`<alias>`, `<alias>.conf`, `<alias>.rhai`) using the first
non-empty, non-comment line as command + args.

## Reconcile

Optional runtime patch phase powered by Rhai. Can modify args/env at launch.

## Patch map

Rhai return object containing supported keys such as `append_args`,
`replace_args`, `set_env`, and `remove_env`.

## Journal routing

Optional stderr forwarding behavior through `systemd-cat --namespace=...`
configured via `[journal]`.

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
