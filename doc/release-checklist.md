# chopper release checklist

Use this checklist before cutting a release tag.

---

## Code quality gates

- [ ] `cargo fmt` has been run
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test -- --nocapture` passes

---

## Behavior sanity checks

- [ ] direct invocation works: `chopper <alias>`
- [ ] symlink invocation works via symlink basename alias
- [ ] built-ins (`--help`, `--version`, print dirs) behave as documented
- [ ] cache bypass works with `CHOPPER_DISABLE_CACHE=1`
- [ ] reconcile bypass works with `CHOPPER_DISABLE_RECONCILE=1`
- [ ] falsey/blank/unknown disable-flag values keep cache + reconcile enabled

---

## Documentation checks

- [ ] root `README.md` reflects current quickstart/install usage
- [ ] [`../README.md`](../README.md) links all relevant docs
- [ ] [`examples.md`](examples.md) and [`templates/`](templates) remain consistent
- [ ] [`operational-spec.md`](operational-spec.md) reflects current semantics for changed behavior

---

## Packaging / release prep

- [ ] `Cargo.toml` version is correct for release
- [ ] release tag `vX.Y.Z` is prepared to match `Cargo.toml`
- [ ] changelog/release notes drafted (optional; GitHub can auto-generate notes)
- [ ] git working tree is clean
- [ ] release commit/tag message prepared
- [ ] pushing the release tag will trigger GitHub Actions to run checks, build the Linux bundle, and publish the GitHub Release

---

## See also

- [`testing.md`](testing.md)
- [`troubleshooting.md`](troubleshooting.md)
- [`docs index`](README.md)
- [`root README`](../README.md)
