# chopper project status

Current implementation status against the DSL implementation goals.

---

## Core feature status

- ✅ Concrete TOML alias DSL
- ✅ Per-alias env injection (`[env]`)
- ✅ Per-alias static args (`args`)
- ✅ Journald namespace stderr routing (`[journal]`)
- ✅ Dynamic user-scoped journal namespace derivation (`journal.user_scope`)
- ✅ Optional journal broker preflight (`journal.ensure`)
- ✅ Optional Rhai runtime reconciliation (`[reconcile]`)
- ✅ Legacy one-line alias compatibility
- ✅ Automatic manifest caching + invalidation
- ✅ Cache corruption/staleness self-healing behavior
- ✅ Direct + symlink invocation support
- ✅ Rhai facade APIs (platform, cap-std fs, duct process, web fetch, SOAP helper)
- ✅ Alias admin CLI (`--alias list|get|add|set|remove`)
- ✅ Interactive TUI (`--tui`) with `(n)vim` Rhai editor integration

---

## Hardening status

- ✅ String validation/normalization policy documented and tested
- ✅ NUL/blank/unsafe structural rejection paths covered
- ✅ Symbolic/path-like shape preservation covered
- ✅ CRLF + mixed-whitespace env flag/override handling covered
- ✅ Disable-flag truthy/falsey/unknown token matrix covered end-to-end
- ✅ ASCII-only truthy token matching behavior covered (unicode lookalikes stay unknown)
- ✅ Broad wrapper/invocation-shape e2e coverage for built-ins and routing
- ✅ Alias lifecycle e2e coverage (add/set/remove clean/remove dirty)

---

## Docs status

- ✅ concise quickstart README
- ✅ docs entrypoint and role-based start-here routing
- ✅ full operational specification
- ✅ CLI/config/testing/troubleshooting/config references
- ✅ examples + templates + migration + FAQ + glossary docs
- ✅ contributor, architecture, decision-log, and release-checklist docs

---

## Remaining implementation work

At this time: **no major feature gaps are identified** against the planned DSL
scope. Ongoing work is primarily doc UX polish and optional future iteration.

---

## How to verify status locally

Use these commands in your checkout:

```bash
git log --oneline -n 20
cargo test -- --nocapture
cargo clippy --all-targets --all-features -- -D warnings
```

This gives you:

- recent implementation history on the active branch
- current behavioral regression status
- lint/quality gate status
