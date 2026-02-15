# chopper template files

Starter files for common setups:

- `basic-alias.toml`
  - minimal alias with simple env injection
- `journal-alias.toml`
  - alias with journald namespace stderr routing
- `reconcile-alias.toml`
  - alias wired to Rhai reconcile script
- `reconcile-script.rhai`
  - example reconcile function with runtime-based patches

Copy these into your config root (typically
`${XDG_CONFIG_HOME:-~/.config}/chopper/aliases`) and adapt as needed.
