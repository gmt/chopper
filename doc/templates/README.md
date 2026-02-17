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
- `bashcomp-alias.toml`
  - alias with `[bashcomp]` configuration options
- `bashcomp-custom.bash`
  - example custom bash completion script for a chopper alias
- `bashcomp-rhai.rhai`
  - example Rhai completion script for a chopper alias
- `rhai-facade-demo.rhai`
  - example reconcile script using platform/fs/process facade APIs

Copy these into your config root (typically
`${XDG_CONFIG_HOME:-~/.config}/chopper/aliases`) and adapt as needed.

Quick copy example:

```bash
mkdir -p ~/.config/chopper/aliases
cp doc/templates/basic-alias.toml ~/.config/chopper/aliases/hello.toml
chopper hello
```

If you use one-off debug toggles while validating templates, only truthy values
disable features (`1`, `true`, `yes`, `on`; trimmed/ASCII case-insensitive).
Falsey values (`0`, `false`, `no`, `off`), blank, and unknown values
(including CRLF-wrapped strings) keep features enabled.
