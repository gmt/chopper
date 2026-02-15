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

---

## Documentation checks

- [ ] root `README.md` reflects current quickstart/install usage
- [ ] `doc/README.md` links all relevant docs
- [ ] `doc/examples.md` and `doc/templates/` remain consistent
- [ ] `doc/operational-spec.md` reflects current semantics for changed behavior

---

## Packaging / release prep

- [ ] `Cargo.toml` version is correct for release
- [ ] changelog/release notes drafted (if used)
- [ ] git working tree is clean
- [ ] release commit/tag message prepared

---

## See also

- [`testing.md`](testing.md)
- [`troubleshooting.md`](troubleshooting.md)
- [`README.md`](README.md)
