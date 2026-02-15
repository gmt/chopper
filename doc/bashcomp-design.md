# Bash completion design for chopper

Research findings, design patterns, and rationale for the `--bashcomp` feature.

---

## Table of contents

- [Problem statement](#problem-statement)
- [Research: scop/bash-completion internals](#research-scopbash-completion-internals)
- [Research: decorator/proxy pattern](#research-decoratorproxy-pattern)
- [Research: cykerway/complete-alias](#research-cykerwaycomplete-alias)
- [Research: Cobra and clap\_complete dynamic models](#research-cobra-and-clap_complete-dynamic-models)
- [Research: Nix and Debian alternatives](#research-nix-and-debian-alternatives)
- [Performance rules](#performance-rules)
- [Self-healing and graceful degradation](#self-healing-and-graceful-degradation)
- [Chopper design decisions](#chopper-design-decisions)

---

## Problem statement

Chopper wraps arbitrary executables via symlink aliases. Bash completion is
fundamentally **early-binding** (compspecs are registered at shell startup or
on first TAB via lazy-loading), but chopper's wrapper behavior is
**late-binding** (the target executable, argument munging, and even whether
completion is supported can change at any time via config edits).

We need a completion strategy that:

1. Delegates to the underlying command's native completer when possible.
2. Adapts gracefully when aliases are added, removed, or retargeted.
3. Avoids blocking, hanging, or degrading the user's shell.
4. Integrates with the standard `bash-completion` framework where available.
5. Works on systems that lack `bash-completion` entirely.
6. Respects per-alias configuration (disabled, passthrough, custom script).

---

## Research: scop/bash-completion internals

### Lazy-loading via `complete -D`

The bash-completion project registers a default compspec:

    complete -F _completion_loader -D

When the user TABs on an unknown command, `_completion_loader` fires. It calls
`__load_completion <command>`, which searches for completion files named
`<command>`, `<command>.bash`, or `_<command>` in a cascade of directories:

1. `$BASH_COMPLETION_USER_DIR/completions/`
2. Directories from `$XDG_DATA_DIRS` with `bash-completion/completions` appended
3. `$BASH_COMPLETION_COMPAT_DIR` (defaults to `/etc/bash_completion.d`)

The user completion directory defaults to:

    ${BASH_COMPLETION_USER_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion}/completions/

Once a file is sourced, it registers a real compspec (e.g.,
`complete -F _comp_cmd_kubectl kubectl`), and completion retries with the
now-registered function.

### `_comp_realcommand` and command resolution

`_comp_realcommand` uses `type -P` to find the real executable path for a
command name, bypassing shell functions, builtins, and aliases. This is how
bash-completion resolves symlinks (e.g., `/usr/bin/editor` via
`/etc/alternatives/editor` to `/usr/bin/vim`).

Key implication for chopper: since all chopper aliases are symlinks to the
same `chopper` binary, `_comp_realcommand <alias>` resolves to the chopper
binary path. This means a `chopper` or `chopper.bash` completion file in the
system completions directory could catch all alias completions on systems
using the standard framework.

### `BASH_COMPLETION_USER_DIR`

User-writable completion directory. Files placed here are discovered by the
lazy-loader on first TAB for the matching command name. This is the right
place for chopper to project per-alias completion shims.

### `_command_offset N`

Built-in decorator for meta-commands (`sudo`, `env`, `nice`, `nohup`). Skips
the first N words in `COMP_WORDS` and delegates completion to whatever command
follows. This is the standard pattern for wrapper completion.

---

## Research: decorator/proxy pattern

The core technique for proxying completion from a wrapper to an underlying
command involves rewriting the bash completion context variables:

    COMP_WORDS[0]  -- the command name being completed
    COMP_CWORD     -- index of the word being completed
    COMP_LINE      -- the full command line string
    COMP_POINT     -- cursor position in COMP_LINE

The `sudo`/`env` pattern:

    _complete_wrapper() {
        local underlying="real-command"
        local orig_words=("${COMP_WORDS[@]}")
        COMP_WORDS[0]="$underlying"
        COMP_LINE="${underlying}${COMP_LINE#"${orig_words[0]}"}"
        COMP_POINT=$(( COMP_POINT - ${#orig_words[0]} + ${#underlying} ))
        # Load underlying completer if needed
        if ! declare -F "_comp_cmd_${underlying##*/}" &>/dev/null; then
            _completion_loader "${underlying##*/}" 2>/dev/null
        fi
        _command_offset 0
        COMP_WORDS=("${orig_words[@]}")
    }
    complete -F _complete_wrapper -o bashdefault -o default my-wrapper

Important: `_command_offset 0` is bash-completion specific. On systems
without bash-completion, we fall back to `compgen -o default`.

---

## Research: cykerway/complete-alias

The `complete-alias` project provides general alias completion by:

1. Saving "vanilla" compspecs in an associative array before aliases mask them.
2. Expanding aliases recursively (with cycle detection via reference counting).
3. Temporarily unmasking the alias (restoring vanilla compspec).
4. Calling `_command_offset 0` to delegate.
5. Re-masking the alias.

Key takeaway: explicit completion registration for aliases is the only
reliable approach. Bash's built-in alias completion resolution (a last-resort
fallback since bash 5.x) is unreliable, especially for multi-word aliases
or aliases with flags.

---

## Research: Cobra and clap_complete dynamic models

### Cobra (Go CLI framework)

Cobra adds hidden `__complete` and `__completeNoDesc` subcommands. The
generated bash completion script calls `$program __complete <partial-args>`
on each TAB press. The binary itself computes completions and returns them
with a `ShellCompDirective` bitmask controlling behavior (error, no-space,
no-file-comp, filter-file-ext).

### clap_complete (Rust)

Supports static generation (compile-time) and dynamic runtime completion via
`CompleteEnv`. The dynamic model sets `COMPLETE=bash` in the environment and
re-invokes the binary; the binary detects this, generates completions, and
exits before heavy initialization.

Key pattern: `CompleteEnv::complete()` runs before application logic.

### Relevance to chopper

Chopper's introspection builtins (`--print-exec`, `--print-bashcomp-mode`)
follow the same principle: fast, deterministic queries that exit before any
heavy work (no Rhai execution, no reconcile, no journald setup).

---

## Research: Nix and Debian alternatives

### Debian alternatives (`/etc/alternatives`)

Uses a two-level symlink chain:

    /usr/bin/editor -> /etc/alternatives/editor -> /usr/bin/vim

Completion works transparently because `type -P editor` resolves through
symlinks and `__load_completion` searches for completions matching both the
symlink name and the resolved binary name.

Challenge for chopper: our symlinks all resolve to the same `chopper` binary,
so the alternatives pattern does not directly help. We need explicit per-alias
completion registration.

### Nix/NixOS

Nix has known gaps with bash completion for wrapped binaries:

- `nix shell` and `nix-shell -p` do not properly expose `XDG_DATA_DIRS`,
  so bash-completion cannot find completion files from Nix store paths.
- The `installShellCompletion` hook installs to `$out/share/bash-completion/completions/`.
- `nix profile` recently added `XDG_DATA_DIRS` support.
- Workaround: manually extend `XDG_DATA_DIRS` in shell hooks.

Lesson for chopper: do not rely on `XDG_DATA_DIRS` propagation. Project
completion shims directly into `BASH_COMPLETION_USER_DIR` where we control
the path.

---

## Performance rules

These rules apply to any code running in the completion path (i.e., every
TAB press):

1. **No network calls.** Completion must work offline and return in <100ms.
2. **No slow external commands.** Avoid `which` (use `type -P`, a builtin).
   Avoid `find`, `grep`, `awk` where shell builtins suffice.
3. **No Rhai execution in the hot path.** Reconcile scripts can be arbitrarily
   slow. Completion queries must bypass reconcile entirely.
4. **Cache in shell variables, not files.** Filesystem I/O adds latency.
   Per-session caching in `declare` variables is effectively free.
5. **Guard expensive paths.** Use `type -P chopper &>/dev/null` before
   invoking chopper introspection. Fall back immediately if chopper is gone.
6. **Avoid blocking.** If any external command might hang (e.g., NFS mount,
   dead network share), use timeouts or avoid the call entirely.

---

## Self-healing and graceful degradation

The completion system must handle these scenarios without manual intervention:

| Scenario | Behavior |
| --- | --- |
| `chopper` binary removed from PATH | Falls back to filename completion |
| Alias config file deleted | `--print-exec` returns error; falls back to filename completion |
| Alias retargeted to different exec | Next completion (or new shell) picks up new target |
| Underlying command has no completer | Falls back to filename completion via `-o default` |
| bash-completion framework not installed | Uses `compgen -o default` fallback |
| `BASH_COMPLETION_USER_DIR` not writable | Shim projection silently skipped |
| Rapid successive TAB presses | Session cache prevents repeated subprocess calls |

Implementation strategy:

- Never hardcode alias-to-exec mappings in the completion script.
- Query chopper introspection dynamically, with per-session caching.
- Guard every external call with existence checks (`type -P`, `declare -F`).
- Use `-o bashdefault -o default` on all `complete` registrations for
  automatic fallback.

---

## Chopper design decisions

### `complete -F` over `complete -C`

`complete -F` (function) runs in-process with full access to `COMP_WORDS`,
`COMP_CWORD`, `COMP_LINE`, `COMP_POINT`. `complete -C` (command) forks a
subprocess per TAB press and only receives `$1` (command), `$2` (current
word), `$3` (previous word). The decorator pattern requires the full context,
so `-F` is the only viable option.

### Late-binding via introspection

The completion script never bakes in alias-to-exec mappings. It queries
`chopper --print-exec <alias>` and `chopper --print-bashcomp-mode <alias>`
at runtime (with session caching). This solves the early-binding vs
late-binding tension: the completion script is static and never stale, while
the data it operates on is always current.

### Session-scoped caching

Completion state is stored in shell variables (`_CHOPPER_EXEC_<alias>`,
`_CHOPPER_MODE_<alias>`) for the shell session lifetime. This avoids
filesystem I/O on every TAB press while still picking up config changes on
new shell sessions. Users can call `_chopper_cache_bust` to force a refresh
within the current session.

### Disabled mode is truly inert

When `bashcomp.disabled = true`, the completion function returns immediately
(`COMPREPLY=(); return 0`) with zero subprocess invocations, zero file reads,
zero blocking. This is critical for aliases that wrap commands with
pathologically slow or broken completion.

### Shim projection is best-effort

Writing per-alias completion shims into `BASH_COMPLETION_USER_DIR` is
opportunistic. If the directory does not exist or is not writable, projection
is silently skipped. The main `_chopper_complete` function works regardless,
as long as `complete -F _chopper_complete <alias>` is registered (which the
sourced `--bashcomp` script handles directly).

### Rhai-based completion (`--complete`)

For aliases where the underlying command has no usable completer, or where
argument transformation makes upstream completion meaningless, chopper
supports Rhai-based completion via the `--complete` introspection command.

This follows the Cobra/clap_complete pattern: the binary itself computes
completions when asked. The bash completion script calls
`chopper --complete <alias> <cword> -- <words...>` and reads candidates
from stdout.

Configuration:

    [bashcomp]
    rhai_script = "completions/myalias.rhai"
    rhai_function = "complete"   # optional, default "complete"

The Rhai function receives a context map (`words`, `cword`, `current`,
`exec`, `alias_args`, `alias_env`) and returns an array of candidate
strings. The completion script (`src/completion.rs`) handles Rhai engine
setup, context building, and candidate extraction.

This is an explicit opt-in that relaxes the "no Rhai in the hot path"
constraint for aliases that specifically request it. The Rhai function is
expected to be lightweight and return in <100ms. Session caching of
mode/exec metadata still applies; only the candidate generation invokes
Rhai per TAB press.

---

## See also

- [`cli-reference.md`](cli-reference.md) for `--bashcomp` usage
- [`operational-spec.md`](operational-spec.md) for `[bashcomp]` config spec
- [`architecture.md`](architecture.md) for module overview
- [scop/bash-completion](https://github.com/scop/bash-completion)
- [cykerway/complete-alias](https://github.com/cykerway/complete-alias)
- [Cobra shell completion guide](https://cobra.dev/docs/how-to-guides/shell-completion)
- [clap_complete docs](https://docs.rs/clap_complete/)
