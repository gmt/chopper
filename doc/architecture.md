# chopper architecture overview

High-level map of the runtime flow and main modules.

---

## Execution pipeline

1. Determine invocation mode (direct vs symlink-derived alias name).
2. Resolve alias config path using lookup order.
3. Load manifest from cache (or parse source on miss/stale/corruption).
4. Optionally apply Rhai reconcile patch.
5. Execute target command:
   - direct `exec` fast path, or
   - journald stderr routing path when configured.

---

## Key modules

- `src/main.rs`
  - CLI entrypoint
  - invocation-mode detection and alias argument extraction
  - config/cache dir resolution and built-ins
  - top-level orchestration (`resolve -> load/parse -> reconcile -> execute`)

- `src/parser.rs`
  - TOML DSL parsing
  - normalization and path-resolution rules
  - field-level validation handoff

- `src/manifest.rs`
  - core manifest data model (`Invocation`, `JournalConfig`, `ReconcileConfig`,
    `BashcompConfig`)
  - deterministic merge helpers for args/env mutation

- `src/cache.rs`
  - manifest cache load/store
  - source fingerprint checks and invalidation
  - corruption/staleness pruning and self-healing behavior

- `src/reconcile.rs`
  - optional Rhai script execution
  - reconcile patch extraction/validation
  - runtime arg/env patch application
- `src/rhai_engine.rs` / `src/rhai_facade/*`
  - shared Rhai engine construction by profile (`Reconcile` vs `Completion`)
  - high-level facade registration (platform/fs/process/web/soap)
  - completion profile exposes safe subset only
- `src/rhai_api_catalog.rs`
  - authoritative API name catalog used for editor completion bootstrap

- `src/executor.rs` / journal execution path
  - `systemd-cat --namespace=...` integration
  - user-scoped namespace derivation (`u<uid>-<user>-<logical>`, default)
  - optional D-Bus broker preflight (`com.chopperproject.JournalBroker1`)
  - journal policy options (`max_use`, `rate_limit_interval_usec`, `rate_limit_burst`)
  - stderr forwarding path and child exit propagation

- `src/journal_broker_client.rs`
  - D-Bus client for `chopper-journal-broker` daemon
  - `ensure_namespace_via_dbus()` method call

- `src/broker/` (broker daemon)
  - `dbus_interface.rs` — D-Bus object implementing `EnsureNamespace`
  - `policy.rs` — namespace ownership validation, anti-abuse limits
  - `systemd.rs` — journald drop-in writing, namespace socket management

- `src/env_util.rs`
  - env toggle/override parsing (`CHOPPER_*`)
  - trimmed, ASCII case-insensitive truthy handling

- `src/bashcomp.bash`
  - static bash completion script, embedded via `include_str!`
  - decorator/proxy pattern delegating to underlying command completers
  - per-session caching, self-healing, graceful degradation
  - see [`bashcomp-design.md`](bashcomp-design.md) for design rationale

- `src/completion.rs`
  - Rhai-based completion engine for `--complete` introspection
  - builds context map, calls Rhai function, returns candidate strings
  - opt-in per-alias (requires `bashcomp.rhai_function`; script path is deterministic `<alias>.rhai`)
- `src/alias_admin.rs` / `src/alias_doc.rs`
  - alias lifecycle operations (`--alias get|add|set|remove`, plus `--list-aliases`)
  - mutation parsing/validation and TOML persistence for managed aliases
- `src/tui.rs` / `src/tui_nvim.rs`
  - alias-first interactive terminal UI (`--tui`) with event-driven key handling
  - ratatui frame rendering on a crossterm backend with content-driven split/modal fallback, compact tab chrome, and alias-list scrollbar support
  - direct `(n)vim` launch/bootstrap and Rhai API completion dictionary generation for Rhai editing

- validation helpers
  - `alias_validation.rs`
  - `arg_validation.rs`
  - `env_validation.rs`
  - `journal_validation.rs`
  - centralized string/shape constraints for parser, cache, reconcile, runtime

---

## Testing layers

- Unit tests in `src/*`
  - parser, validation, env parsing, reconcile contract, cache mechanics
- End-to-end tests in `tests/e2e.rs`
  - invocation forms, cache lifecycle, journal behavior, reconcile behavior,
    override handling, and edge-case string-shape coverage

---

## Design intent

- Keep simple aliases simple (small files, fast path execution).
- Keep advanced behavior explicit (journal and reconcile opt-in).
- Reject only structurally unsafe strings; preserve useful symbolic/path shapes.
- Make cache behavior transparent and self-healing.

---

## See also

- [`testing.md`](testing.md)
- [`operational-spec.md`](operational-spec.md)
- [`bashcomp-design.md`](bashcomp-design.md)
- [`docs index`](README.md)
- [`root README`](../README.md)
