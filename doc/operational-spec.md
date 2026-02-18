# chopper Operational Specification

This document contains the detailed behavioral specification for `chopper`,
including edge-case semantics, validation rules, and cache/runtime behavior.

For a concise overview and quickstart, see the root `README.md`.

---

## Section map

- [Installation / invocation model](#installation--invocation-model)
- [Alias config discovery](#alias-config-discovery)
- [DSL reference (TOML)](#dsl-reference-toml)
  - [Parsing / validation rules](#parsing--validation-rules)
  - [String-shape policy (what is intentionally allowed)](#string-shape-policy-what-is-intentionally-allowed)
  - [Argument merge order](#argument-merge-order)
  - [Environment merge order](#environment-merge-order)
- [Journald namespace behavior](#journald-namespace-behavior)
- [Optional runtime reconciliation (Rhai)](#optional-runtime-reconciliation-rhai)
- [Alias administration CLI](#alias-administration-cli)
- [Rhai facade APIs](#rhai-facade-apis)
- [Terminal UI](#terminal-ui)
- [Bash completion](#bash-completion)
- [Caching](#caching)

---

## Installation / invocation model

`chopper` supports two invocation styles:

1. **Direct**:

```bash
chopper <alias> [args...]
```

You may insert `--` to separate chopper parsing from alias args:

```bash
chopper <alias> -- [args...]
```

Only one leading `--` separator is consumed; additional `--` tokens are passed
through as normal runtime arguments.

Built-in flags for direct invocation:

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
chopper --complete <alias> <cword> [--] <words...>
chopper --alias <list|get|add|set|remove> ...
chopper --tui [--tmux=<auto|on|off>] [--no-tmux]
```

A binary named `chopper.exe`, `chopper.com`, `chopper.cmd`, or `chopper.bat`
is treated the same as `chopper` for direct invocation and built-in detection
(including when `argv[0]` is provided as a full path with `/` or `\`
separators). This executable-name detection is ASCII case-insensitive
(`chopper`, `CHOPPER`, `CHOPPER.EXE`, `CHOPPER.COM`, `CHOPPER.CMD`,
`CHOPPER.BAT`, etc.).

Windows-relative launcher shapes such as `.\CHOPPER.CMD` and
`..\CHOPPER.BAT` are also treated as direct invocation names. UNC-style
launcher paths such as `\\server\tools\CHOPPER.COM` are treated the same way.
Drive-letter launcher paths like `C:\tools\CHOPPER.EXE` are likewise treated
as direct invocation names. Unix-relative launcher paths such as
`./CHOPPER.COM` and `../CHOPPER.CMD` are also treated as direct invocation
names.

Equivalent forward-slash Windows spellings (for example
`C:/tools/CHOPPER.CMD` and `//server/tools/CHOPPER.COM`) are recognized as
well. Mixed-separator launcher paths (for example `C:/tools\CHOPPER.COM` and
`\\server/tools\CHOPPER.BAT`) are recognized too. Nested relative variants
with mixed separators (for example `./nested\CHOPPER.CMD`) are recognized as
direct invocation names as well. Trailing path separators on launcher paths
(for example `C:/tools/CHOPPER.CMD/`) are tolerated for direct invocation
detection. Mixed absolute forms that combine Unix and Windows separators (for
example `/tmp\CHOPPER.CMD`, `/tmp\CHOPPER`, or `/tmp\CHOPPER/`) are also
recognized.

Built-ins are single-action commands. Additional positional tokens are normally
treated as regular alias parsing input and therefore should not be provided.
`--tui` is the exception and accepts TUI option flags only.

1. **Symlinked alias**:

```bash
ln -s /path/to/chopper /usr/local/bin/kpods
kpods [args...]
```

In symlink mode, alias name is inferred from executable name (`kpods` above).
You may also use `kpods -- [args...]` to explicitly separate passthrough args.
The symlink basename is used verbatim (including dots like `kpods.prod`).
Built-in flags such as `--help`, `-h`, `--version`, and `-V` are treated as
normal passthrough arguments in symlink mode, including print-path flags such
as `--print-config-dir` and `--print-cache-dir`.

Alias names in direct mode are logical identifiers (not filesystem paths), so
path separators, whitespace, NUL bytes, and dash-prefixed tokens are rejected.
The same alias-identifier validation rules are applied in symlink mode.
Dot-separated names like `demo.prod` are valid in both direct and symlink
modes. Other punctuation tokens (for example `alpha:beta`) are also supported
as long as they satisfy direct-mode validation rules above. Unicode alias names
are also allowed (for example `emoji🚀`) if they satisfy the same validation
constraints.

---

## Alias config discovery

Config root is `${XDG_CONFIG_HOME:-~/.config}/chopper`.

For advanced scenarios, you can override config root explicitly:

```bash
CHOPPER_CONFIG_DIR=/path/to/config-root chopper <alias> [args...]
```

When this override is set, paths are resolved directly under that root.
Leading/trailing whitespace wrappers are trimmed, but internal path shape is
preserved (including symbolic/unicode segments, UNC-like strings, mixed
separators, and trailing separators). Blank values are ignored and fall back to
XDG/default resolution.

Lookup order for alias `foo`:

1. `aliases/foo.toml`
2. `foo.toml`

Only regular files are considered valid alias configs in this lookup. Symlinks
that resolve to regular files are accepted.

Legacy one-line alias files are no longer parsed. Alias configs must be TOML.

Configuration-oriented flows (`--list-aliases`, `--alias ...`, and TUI scans)
emit warnings for suspicious config files with extensions other than `.toml`
or `.rhai`. These diagnostics are advisory and do not alter alias invocation
semantics.

---

## DSL reference (TOML)

```toml
exec = "kubectl"                 # required
args = ["get", "pods", "-A"]     # optional, default [] (NUL bytes rejected)
env_remove = ["AWS_PROFILE"]     # optional, default []

[env]                            # optional map<string,string>
KUBECONFIG = "/home/me/.kube/config"

[journal]                        # optional
namespace = "ops"                # required when [journal] is present
stderr = true                    # optional, default true
identifier = "kpods"             # optional (blank values are treated as unset)

[reconcile]                      # optional
script = "kpods.reconcile.rhai"  # required
function = "reconcile"           # optional, default "reconcile"

[bashcomp]                       # optional
disabled = false                 # optional, default false
passthrough = false              # optional, default false
script = "comp/kpods.bash"       # optional custom completion script
rhai_script = "comp/kpods.rhai"  # optional Rhai completion script
rhai_function = "complete"       # optional, default "complete"
```

### Parsing / validation rules

- Leading/trailing whitespace in string fields like `exec`,
  `journal.namespace`, `journal.identifier`, `reconcile.script`, and
  `reconcile.function` is trimmed.
- Those string fields cannot contain NUL bytes.
- `args` entries cannot contain NUL bytes.
- `env_remove` entries are trimmed, deduplicated (first-seen order), and blank
  entries are ignored.
- `env_remove` entries cannot contain `=` or NUL bytes.
- `[env]` keys are trimmed and must remain unique after trimming.
- `[env]` keys cannot contain `=` or NUL bytes.
- `[env]` values cannot contain NUL bytes.
- `exec` cannot be `.` or `..`.
- Relative `exec` forms like `./` or `.\` must include a path segment (for
  example `./bin/tool`).
- `exec` cannot end with a path separator (relative or absolute).
- `exec` cannot end with `.` or `..` path components (for example `bin/..`).
- If `exec` is a relative path (for example `bin/runner`), it is resolved
  against the alias config file's real directory (following symlinks).
- TOML documents may optionally start with a UTF-8 BOM.

### String-shape policy (what is intentionally allowed)

`chopper` intentionally rejects only values that are structurally unsafe for
process execution and environment mutation:

- NUL bytes in command/arg/env/journal/reconcile string fields
- `=` in env keys / env_remove / remove_env entries
- blank-after-trim values for fields that require non-empty tokens

Other symbolic/path-like shapes are intentionally preserved (for example
slashes, backslashes, dots, dashes, braces, dollar signs, and semicolons),
including for alias args, reconcile args, env values, env keys, env_remove
entries, and journal namespace/identifier.

When authoring TOML that contains backslashes, prefer literal strings
(`'windows\path'`) if you want raw backslashes preserved exactly.

### Argument merge order

1. alias `args`
2. runtime args passed at invocation time
3. optional Rhai patch (`replace_args`, then `append_args`)

All argument channels reject NUL bytes.

### Environment merge order

1. process inherits parent environment
2. alias `[env]` is injected
3. alias `env_remove` is removed
4. optional Rhai patch (`set_env`, then `remove_env`)

This means reconcile `set_env` can intentionally re-introduce a key that alias
`env_remove` removed, while reconcile `remove_env` still has final precedence.

---

## Journald namespace behavior

When `[journal]` is configured with `stderr = true`, `chopper`:

- launches `systemd-cat --namespace=...` first
- verifies the journal sink is alive before launching the target command
- launches target command only after journal sink startup succeeds
- captures target stderr
- forwards stderr into `systemd-cat --namespace=<namespace>`
- keeps stdout attached normally

If `systemd-cat` is missing or does not support `--namespace` (systemd < 256),
execution fails with an explicit error.

---

## Optional runtime reconciliation (Rhai)

When `[reconcile]` is set, the script function is called with:

- `runtime_args`
- `runtime_env`
- `alias_args`
- `alias_env`

The function must return a map. Supported keys:

- `append_args: [string]`
- `replace_args: [string]`
- `set_env: #{ string: string }`
- `remove_env: [string]`

Unknown keys are rejected to catch script typos early.

For reconcile env mutations, `set_env` keys and `remove_env` entries are
trimmed; blank keys are rejected, and `remove_env` entries are deduplicated
(first-seen order) while blank remove entries are ignored.

- `append_args` and `replace_args` entries cannot contain NUL bytes.
- `set_env` keys cannot contain `=` or NUL bytes.
- `set_env` values cannot contain NUL bytes.
- `remove_env` entries cannot contain `=` or NUL bytes.
- Relative `reconcile.script` paths are resolved against the alias config
  file's real directory (following symlinks).
- `reconcile.script` cannot be `.` or `..`.
- Relative forms like `./` or `.\` must include a script path segment (for
  example `./hooks/reconcile.rhai`).
- `reconcile.script` cannot end with a path separator (relative or absolute).
- `reconcile.script` cannot end with `.` or `..` path components.

For extraordinary debugging scenarios, reconcile can be bypassed
per-invocation:

```bash
CHOPPER_DISABLE_RECONCILE=1 chopper <alias> [args...]
```

`CHOPPER_DISABLE_RECONCILE` uses trimmed, ASCII case-insensitive truthy parsing:
`1`, `true`, `yes`, and `on` disable reconcile. Blank, falsey (`0`, `false`,
`no`, `off`), or unknown values leave reconcile enabled. This includes
whitespace/CRLF-wrapped unknown values such as `"\r\nmaybe\r\n"`.
Non-ASCII lookalike tokens (for example fullwidth `ＴＲＵＥ`) are treated as
unknown and therefore leave reconcile enabled.

Examples that **leave reconcile enabled**:

- `CHOPPER_DISABLE_RECONCILE="\r\n0\r\n"`
- `CHOPPER_DISABLE_RECONCILE="\r\nfalse\r\n"`
- `CHOPPER_DISABLE_RECONCILE="\r\nno\r\n"`
- `CHOPPER_DISABLE_RECONCILE="\r\noff\r\n"`
- `CHOPPER_DISABLE_RECONCILE="\r\n\u00A0FaLsE\u00A0\r\n"` (CRLF + NBSP-wrapped falsey token)
- `CHOPPER_DISABLE_RECONCILE="\u3000FaLsE\u3000"` (ideographic-space wrapped falsey token)
- `CHOPPER_DISABLE_RECONCILE="\r\n   \r\n"`
- `CHOPPER_DISABLE_RECONCILE="\t\t"` (tab-only blank)
- `CHOPPER_DISABLE_RECONCILE="Ｔrue"` (mixed-script lookalike)
- `CHOPPER_DISABLE_RECONCILE="\r\nＴrue\r\n"` (wrapped mixed-script lookalike)
- `CHOPPER_DISABLE_RECONCILE="\u3000Ｔrue\u3000"` (ideographic-space wrapped mixed-script lookalike)
- `CHOPPER_DISABLE_RECONCILE="\r\n\u00A0Ｔrue\u00A0\r\n"` (CRLF + NBSP-wrapped mixed-script lookalike)

---

## Alias administration CLI

`chopper` includes an alias lifecycle command family:

```bash
chopper --alias list
chopper --alias get <alias>
chopper --alias add <alias> --exec <command> [--arg <arg> ...] [--env KEY=VALUE ...]
chopper --alias set <alias> [--exec <command>] [--arg <arg> ...] [--env KEY=VALUE ...]
chopper --alias remove <alias> [--mode clean|dirty] [--symlink-path <path>]
```

Key semantics:

- `add` writes TOML alias configs under `aliases/<alias>.toml`.
- `set` updates existing TOML alias configs.
- `remove --mode clean` removes config + cache and attempts symlink cleanup.
- `remove --mode dirty` removes symlink only, preserving config for reactivation.

---

## Rhai facade APIs

Rhai scripts can call facade functions for higher-level automation intent:

- platform introspection and executable intent checks
- cap-std based fs inspection/manipulation
- duct based process execution with timeout support
- curl-ish web fetch helpers
- soap envelope + call helpers

See [`rhai-facade-reference.md`](rhai-facade-reference.md) for full catalog.

Facade exposure is profile-aware:

- reconcile profile: full facade set
- completion profile: safe subset only (platform + read-only fs helpers)

---

## Terminal UI

`chopper --tui` opens an interactive terminal workflow that is alias-first:
aliases are listed directly and navigated by arrow/vim directional keys.

The TUI requires an interactive terminal.

The TUI runs in an alternate terminal screen and restores terminal state before
launching external editor subprocesses, then re-enters the interactive view.

Layout behavior:

- Layout is content-driven. Split view (alias list + inspector/details pane) is
  preferred when both panes remain functional without unreasonable truncation.
- If width becomes constrained, tab chrome compacts to the active-tab label.
- If split still cannot remain functional, the UI falls back to a
  modal/single-pane list view with a tab strip row.
- The inspector uses tabs (`toml`, `reconcile`) that are
  always selectable; tabs with backing data are emphasized, while empty tabs
  remain selectable for creation flows.
- The top banner provides concise action guidance (`Enter`, `Tab`, `e`, `r`,
  `q` plus alias ops). Bottom rows are used for async config warnings and
  prompts/errors.
- Alias overflow is represented by a vertical scrollbar.
- In modal fallback, inspector/editor interaction opens as a wizard-like
  full-screen pane while list mode remains available.

Editing behavior:

- `Enter` from list focus moves into inspector focus for the active tab.
- TOML schema-bound fields are edited directly in the TUI inspector (no
  external editor handoff for normal property edits).
- `e` is a reconcile quick action; when reconcile script is missing, the TUI can
  open a draft creation flow.
- Reconcile script draft files include instructional comments and only persist
  if the user saves before exit. Aborting with `:q!` discards draft changes.
- Alias lifecycle actions are available in TUI (`new`, `rename`, `duplicate`,
  `delete`) via prompt-driven controls. Delete prompt supports a `keep configs`
  toggle (`k`) to switch between clean removal and symlink-only removal.
- `--tmux=auto` (default) uses tmux only when appropriate:
  - inside tmux: launches editor directly in the current pane (no split pane)
  - outside tmux with no running server: launches a dedicated tmux session
  - outside tmux with an already-running server: avoids creating a second
    session and falls back to direct (tmuxless) editor launch
- `--tmux=on` forces tmux use and errors when tmux is unavailable; inside tmux
  it uses direct launch in the active pane.
- `--tmux=off` and `--no-tmux` force tmuxless editor launch.

Rhai script editing remains available through the same `(n)vim` integration,
including completion dictionary generation from exposed facade API names.

See [`tui-reference.md`](tui-reference.md) for full workflow details.

---

## Bash completion

When `[bashcomp]` is configured, it controls how bash tab completion behaves
for the alias.

```toml
[bashcomp]
disabled = false       # optional, default false
passthrough = false    # optional, default false
script = "comp/x.bash" # optional custom completion script
```

### `bashcomp.disabled`

When `true`, completion is entirely suppressed for this alias. The completion
function returns immediately with no subprocess invocations, no file reads,
and no blocking of any kind. Bash falls through to its default filename
completion.

Use this for aliases that wrap commands with pathologically slow or broken
completion, or for aliases where completion is not meaningful.

### `bashcomp.passthrough`

When `true`, completion delegates directly to the underlying command's native
completer without applying any Rhai argument transformation. The completion
context is rewritten to reference the underlying `exec`, and the underlying
command's completer handles everything.

### `bashcomp.script`

Optional path to a custom bash completion script. Resolved relative to the
alias config file's real directory (following symlinks), using the same path
resolution rules as `reconcile.script`.

The script must define a function named `_chopper_bashcomp_<alias>()` (with
non-alphanumeric characters replaced by `_`). This function is called instead
of the default delegation logic.

Validation rules for `bashcomp.script`:

- Cannot contain NUL bytes.
- Cannot be `.` or `..`.
- Cannot end with a path separator.
- Cannot end with `.` or `..` path components.
- Relative forms like `./` or `.\` must include a file path segment.
- Blank values are treated as unset.

### `bashcomp.rhai_script`

Optional path to a Rhai script that provides completion logic. Resolved
relative to the alias config file's real directory (following symlinks),
using the same path resolution rules as `reconcile.script`.

The script must define a function (default name: `complete`) that receives
a context map and returns an array of candidate strings.

Validation rules match `bashcomp.script` (NUL rejection, dot/separator
checks, relative path segment requirement).

### `bashcomp.rhai_function`

Optional function name within the `rhai_script`. Defaults to `"complete"`.
Requires `rhai_script` to be set. Trimmed; blank values treated as unset.

### Completion mode precedence

When `--print-bashcomp-mode <alias>` is queried, the mode is determined by:

1. If `bashcomp.disabled` is `true`: `disabled`
2. If `bashcomp.script` is set: `custom`
3. If `bashcomp.rhai_script` is set: `rhai`
4. If `bashcomp.passthrough` is `true`: `passthrough`
5. Otherwise: `normal`

### Rhai completion (`--complete`)

When mode is `rhai`, the bash completion script calls:

```bash
chopper --complete <alias> <cword> -- <words...>
```

This loads the Rhai script from `bashcomp.rhai_script`, calls the named
function with a context map containing:

- `words`: array of strings (COMP_WORDS from bash)
- `cword`: integer (0-based index of word being completed)
- `current`: string (the partial word, i.e. `words[cword]`)
- `exec`: string (resolved exec path for the alias)
- `alias_args`: array of strings (alias's configured args)
- `alias_env`: map of string to string (alias's configured env)

The function returns an array of candidate strings, printed one per line.

This is an opt-in per-alias feature that relaxes the "no Rhai in the hot
path" constraint. The Rhai function should return quickly (<100ms).

### Setup

Enable bash completion by sourcing the output of `chopper --bashcomp`:

```bash
source <(chopper --bashcomp)
```

Or save it persistently:

```bash
chopper --bashcomp > ~/.local/share/bash-completion/completions/chopper
```

The script registers completion handlers for all configured aliases and
projects per-alias shims into `BASH_COMPLETION_USER_DIR` (best-effort).

See [`bashcomp-design.md`](bashcomp-design.md) for the full design rationale.

---

## Caching

Parsed manifests are cached automatically under
`${XDG_CACHE_HOME:-~/.cache}/chopper/manifests`.

For advanced scenarios, cache root can be overridden explicitly:

```bash
CHOPPER_CACHE_DIR=/path/to/cache-root chopper <alias> [args...]
```

Blank values are ignored and fall back to XDG/default resolution. As with
`CHOPPER_CONFIG_DIR`, wrapped whitespace is trimmed while preserving path shape
(for example unicode/symbolic segments, UNC-like strings, mixed separator
strings, and trailing separators).

Cache invalidation is automatic and based on source file path + metadata (size,
mtime, and on Unix also ctime/device/inode). Users do not need to manually
manage cache in normal usage.

If a cache entry is corrupted or contains invalid runtime strings (for example
NUL bytes, empty/whitespace env/env_remove/journal/reconcile metadata, env keys
containing `=`, or invalid exec/reconcile script path forms such as `.`, `..`,
dot-suffixed components, and trailing separators), chopper automatically ignores
and prunes that entry before reparsing the source config. That repaired manifest
is then written back into cache during the same invocation, so subsequent runs
use a clean cache entry again.

Likewise, chopper refuses to write invalid manifests into cache in the first
place, so malformed cache state only persists if files are externally altered.
The same prune/reparse rules apply to both safe aliases (for example
`safealias.bin`) and hashed unsafe-alias cache filenames.

For extraordinary debugging scenarios, cache can be bypassed per-invocation:

```bash
CHOPPER_DISABLE_CACHE=1 chopper <alias> [args...]
```

`CHOPPER_DISABLE_CACHE` uses the same trimmed, ASCII case-insensitive truthy
parsing:
`1`, `true`, `yes`, and `on` disable cache. Blank, falsey (`0`, `false`, `no`,
`off`), or unknown values keep cache enabled. This includes
whitespace/CRLF-wrapped unknown values such as `"\r\nmaybe\r\n"`.
Non-ASCII lookalike tokens (for example fullwidth `ＴＲＵＥ`) are treated as
unknown and therefore keep cache enabled.

Examples that **keep cache enabled**:

- `CHOPPER_DISABLE_CACHE="\r\n0\r\n"`
- `CHOPPER_DISABLE_CACHE="\r\nfalse\r\n"`
- `CHOPPER_DISABLE_CACHE="\r\nno\r\n"`
- `CHOPPER_DISABLE_CACHE="\r\noff\r\n"`
- `CHOPPER_DISABLE_CACHE="\r\n\u00A0FaLsE\u00A0\r\n"` (CRLF + NBSP-wrapped falsey token)
- `CHOPPER_DISABLE_CACHE="\u3000FaLsE\u3000"` (ideographic-space wrapped falsey token)
- `CHOPPER_DISABLE_CACHE="\r\n   \r\n"`
- `CHOPPER_DISABLE_CACHE="\t\t"` (tab-only blank)
- `CHOPPER_DISABLE_CACHE="Ｔrue"` (mixed-script lookalike)
- `CHOPPER_DISABLE_CACHE="\r\nＴrue\r\n"` (wrapped mixed-script lookalike)
- `CHOPPER_DISABLE_CACHE="\u3000Ｔrue\u3000"` (ideographic-space wrapped mixed-script lookalike)
- `CHOPPER_DISABLE_CACHE="\r\n\u00A0Ｔrue\u00A0\r\n"` (CRLF + NBSP-wrapped mixed-script lookalike)
