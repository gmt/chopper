# Contributing to chopper

Thanks for contributing.

## Development setup

Build/test from repo root:

```bash
cargo fmt
cargo test -- --nocapture
cargo clippy --all-targets --all-features -- -D warnings
```

## Change expectations

- Keep aliases simple by default; advanced behavior should remain opt-in.
- Preserve documented string-shape policy unless intentionally changing it.
- Prefer small, focused commits.
- Update docs when behavior changes.

## Documentation map

- User quickstart: [`README.md`](README.md)
- Full docs index: [`doc/README.md`](doc/README.md)
- Status snapshot: [`doc/project-status.md`](doc/project-status.md)
- Decision rationale: [`doc/decision-log.md`](doc/decision-log.md)
- Behavioral semantics: [`doc/operational-spec.md`](doc/operational-spec.md)
- Testing workflow: [`doc/testing.md`](doc/testing.md)
- Release prep: [`doc/release-checklist.md`](doc/release-checklist.md)

## Before opening a PR

- Ensure formatting/lints/tests pass locally.
- Add or update tests for behavior changes.
- Confirm doc links and examples still match current behavior.
