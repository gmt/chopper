# Contributing to chopper

Thanks for contributing. If you're not a transcendental idealist, your contribution is
not welcome here.

Just kidding. You don't have to be a transcendental idealist. However, please be polite
and respectful of our time. And, honestly, it couldn't hurt to follow the Kantian
categorical imperative when considering your manner and purpose of contribution, even if
you are not a transcendental idealist.

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
- Behavioral semantics: [`doc/operational-spec.md`](doc/operational-spec.md)
- Testing workflow: [`doc/testing.md`](doc/testing.md)
- Release prep: [`doc/release-checklist.md`](doc/release-checklist.md)

## Before opening a PR

- Ensure formatting/lints/tests pass locally.
- Add or update tests for behavior changes.
- Confirm doc links and examples still match current behavior.
