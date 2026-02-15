# Chopper agent conventions

Project-level guidance for AI agents working on this codebase.

---

## Key design documents

- [`doc/bashcomp-design.md`](doc/bashcomp-design.md) -- Bash completion
  research findings, design patterns, and rationale. Must be consulted before
  modifying any completion-related code.
- [`doc/operational-spec.md`](doc/operational-spec.md) -- Authoritative
  behavioral specification. All runtime semantics, validation rules, and
  edge-case handling are documented here.
- [`doc/architecture.md`](doc/architecture.md) -- Module responsibilities
  and execution pipeline overview.

---

## Coding conventions

- **Validation separation**: Dedicated `*_validation.rs` modules for field
  constraints. New fields that accept user strings must have corresponding
  validation.
- **NUL byte rejection**: All user-facing string fields reject NUL bytes.
  This is a hard invariant throughout the codebase.
- **Truthy flag parsing**: Environment toggles (`CHOPPER_DISABLE_*`) use
  trimmed, ASCII case-insensitive truthy parsing via `env_util::env_flag_enabled`.
  Truthy values: `1`, `true`, `yes`, `on`. Everything else (including blank,
  falsey, unknown, non-ASCII lookalikes) keeps the feature enabled.
- **Path resolution**: Relative paths in config files resolve against the
  config file's real directory (following symlinks via `fs::canonicalize`).
- **Cache self-healing**: Corrupted or invalid cache entries are automatically
  pruned and rebuilt from source. New cached fields must be validated in
  `cache.rs` `validate_cached_manifest()`.
- **Atomic writes**: Cache writes use temp file + rename for atomicity.
- **Test structure**: Unit tests live in `#[cfg(test)] mod tests` blocks
  within each source file. E2E tests live in `tests/e2e.rs`. Both layers
  must be updated when adding new features.

---

## Bash completion subsystem

The `--bashcomp` feature emits a static bash script (embedded via
`include_str!`) that provides tab completion for all chopper-managed aliases.

Key constraints for completion code:

- **No Rhai in the hot path**: Completion queries (`--print-exec`,
  `--print-bashcomp-mode`) must not execute reconcile scripts. They read
  the manifest only. Exception: aliases with `bashcomp.rhai_script`
  explicitly opt in to Rhai execution via `--complete` per TAB press.
- **No blocking**: The bash completion function must never hang or block.
  All external calls are guarded with existence checks.
- **Session caching**: Completion state is cached in shell variables to
  avoid subprocess overhead on repeated TAB presses.
- **Graceful degradation**: If chopper is missing, if an alias is gone, or
  if the underlying command has no completer, the system falls back to
  filename completion silently.

See [`doc/bashcomp-design.md`](doc/bashcomp-design.md) for the full design
rationale and research references.
