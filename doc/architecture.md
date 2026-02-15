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
  - TOML DSL parsing + legacy one-line fallback parsing
  - normalization and path-resolution rules
  - field-level validation handoff

- `src/manifest.rs`
  - core manifest data model (`Invocation`, `JournalConfig`, `ReconcileConfig`)
  - deterministic merge helpers for args/env mutation

- `src/cache.rs`
  - manifest cache load/store
  - source fingerprint checks and invalidation
  - corruption/staleness pruning and self-healing behavior

- `src/reconcile.rs`
  - optional Rhai script execution
  - reconcile patch extraction/validation
  - runtime arg/env patch application

- `src/journal.rs` / journal execution path
  - `systemd-cat --namespace=...` integration
  - stderr forwarding path and child exit propagation

- `src/env_util.rs`
  - env toggle/override parsing (`CHOPPER_*`)
  - trimmed, case-insensitive truthy handling

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
