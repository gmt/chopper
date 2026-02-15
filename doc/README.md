# chopper docs

Use this directory when you need behavior details beyond quickstart usage.

## If you need...

- **one command quickly** → [`quick-reference.md`](quick-reference.md)
- **a working example to copy** → [`examples.md`](examples.md)
- **starter files to drop in** → [`templates/`](templates)
- **migration steps from legacy aliases** → [`migration.md`](migration.md)
- **help diagnosing a failure** → [`troubleshooting.md`](troubleshooting.md)
- **full exact semantics** → [`operational-spec.md`](operational-spec.md)

## Docs map

- [`operational-spec.md`](operational-spec.md)
  - full behavioral specification
  - invocation edge-cases
  - validation and normalization rules
  - journald and Rhai reconciliation semantics
  - cache lifecycle and self-healing behavior
- [`architecture.md`](architecture.md)
  - runtime flow overview
  - module ownership map
  - testing-layer summary
- [`testing.md`](testing.md)
  - local validation commands
  - targeted/full test run patterns
  - suggested verification sequence
- [`release-checklist.md`](release-checklist.md)
  - pre-release validation checklist
  - docs/runtime sanity gates before tagging
- [`examples.md`](examples.md)
  - copy/paste alias patterns
  - common workflows
  - quick recipes for env/journal/reconcile/legacy usage
- [`migration.md`](migration.md)
  - phased migration from legacy one-line aliases to TOML
  - practical conversion flow for env/journal/reconcile adoption
- [`templates/`](templates)
  - starter files for common alias/reconcile setups
  - quick copy source for first working configs
- [`quick-reference.md`](quick-reference.md)
  - compact command lookup
  - env override/toggle cheat sheet
  - alias discovery path summary
- [`cli-reference.md`](cli-reference.md)
  - command-oriented invocation and env control reference
  - built-ins, toggles, and mode semantics at a glance
- [`faq.md`](faq.md)
  - short answers to common "where do I look?" questions
  - quick links to the right doc for each task
- [`glossary.md`](glossary.md)
  - concise definitions for recurring terms
  - quick semantic lookup while reading deeper docs
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
