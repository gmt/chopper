# chopper docs

Use this directory when you need behavior details beyond quickstart usage.

## Docs map

- [`operational-spec.md`](operational-spec.md)
  - full behavioral specification
  - invocation edge-cases
  - validation and normalization rules
  - journald and Rhai reconciliation semantics
  - cache lifecycle and self-healing behavior
- [`examples.md`](examples.md)
  - copy/paste alias patterns
  - common workflows
  - quick recipes for env/journal/reconcile/legacy usage
- [`quick-reference.md`](quick-reference.md)
  - compact command lookup
  - env override/toggle cheat sheet
  - alias discovery path summary
- [`troubleshooting.md`](troubleshooting.md)
  - common failure modes
  - quick diagnosis checklist
  - where to look first for cache/reconcile/journal/config issues

## Suggested reading path

1. Start at the root `README.md` for setup and common usage.
2. Use `quick-reference.md` for fast command lookup.
3. Use `examples.md` for common copy/paste workflows.
4. Use `troubleshooting.md` if something is not behaving as expected.
5. Use `operational-spec.md` for full edge-case and semantic detail.
