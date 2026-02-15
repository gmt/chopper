# chopper docs

Use this directory when you need behavior details beyond quickstart usage.

## Docs map

- [`operational-spec.md`](operational-spec.md)
  - full behavioral specification
  - invocation edge-cases
  - validation and normalization rules
  - journald and Rhai reconciliation semantics
  - cache lifecycle and self-healing behavior
- [`troubleshooting.md`](troubleshooting.md)
  - common failure modes
  - quick diagnosis checklist
  - where to look first for cache/reconcile/journal/config issues

## Suggested reading path

1. Start at the root `README.md` for setup and common usage.
2. Use `troubleshooting.md` if something is not behaving as expected.
3. Use `operational-spec.md` for full edge-case and semantic detail.
