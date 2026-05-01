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

## Pull requests

For normal contributions, open a pull request against the main branch. You do
not need release credentials, local maintainer scripts, or any machine-specific
setup beyond the Rust toolchain.

Before opening a PR:

- Keep the change focused and explain the user-visible behavior it affects.
- Add or update tests for behavior changes.
- Update docs when behavior changes.
- Run formatting, tests, and clippy locally when practical.

## Maintainer releases

Releases are automated on GitHub Actions.

The low-friction maintainer path is:

```bash
scripts/plbump.sh
```

This script is for maintainers with push/tag permissions. It assumes the
current branch is the release branch, may fold the current worktree into the
release commit, creates and pushes a tag, and then stages the next patch
version. Contributors should not need to run it.

If you want the script to refresh compatible dependency versions during the
release cut, use:

```bash
scripts/plbump.sh --update-deps
```

If you want the release commit to come only from what you have already staged,
use:

```bash
scripts/plbump.sh --index
```

That mode temporarily stashes unstaged and untracked changes, runs the release
from the current index, then restores the unstaged worktree afterward.

Manual flow:

1. Update `Cargo.toml` to the intended release version.
2. Ensure the working tree is clean and the release checklist passes.
3. Push a matching tag such as `v0.99.1`.

That tag triggers the GitHub release workflow, which validates the tag/version
match, runs `fmt`/`clippy`/`test`, builds the Linux release bundle, and
publishes a GitHub Release with generated notes.

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
