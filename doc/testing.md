# chopper testing guide

How to validate behavior locally.

---

## Fast checks

Format:

```bash
cargo fmt
```

Lint (warnings denied):

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Run full test suite:

```bash
cargo test -- --nocapture
```

---

## Targeted test runs

Run a single test by name:

```bash
cargo test <test_name> -- --nocapture
```

Examples:

```bash
cargo test print_dir_builtins_trim_crlf_wrapped_symbolic_override_paths -- --nocapture
cargo test cache_disable_flag_crlf_wrapped_truthy_disables_cache_in_e2e_flow -- --nocapture
cargo test reconcile_disable_flag_crlf_wrapped_falsey_value_keeps_reconcile_enabled -- --nocapture
cargo test cache_disable_flag_crlf_wrapped_unknown_value_keeps_cache_enabled_and_uses_existing_cache_entry -- --nocapture
cargo test reconcile_disable_flag_crlf_wrapped_unknown_value_keeps_reconcile_enabled -- --nocapture
cargo test cache_disable_flag_nbsp_wrapped_truthy_disables_cache_in_e2e_flow -- --nocapture
cargo test reconcile_disable_flag_nbsp_wrapped_truthy_disables_reconcile_in_e2e_flow -- --nocapture
cargo test cache_disable_flag_nbsp_wrapped_false_value_keeps_cache_enabled_and_uses_existing_cache_entry -- --nocapture
cargo test reconcile_disable_flag_nbsp_wrapped_false_value_keeps_reconcile_enabled -- --nocapture
cargo test cache_disable_flag_crlf_and_nbsp_wrapped_truthy_disables_cache_in_e2e_flow -- --nocapture
cargo test reconcile_disable_flag_crlf_and_nbsp_wrapped_truthy_disables_reconcile_in_e2e_flow -- --nocapture
cargo test cache_disable_flag_crlf_and_nbsp_wrapped_false_value_keeps_cache_enabled_and_uses_existing_cache_entry -- --nocapture
cargo test reconcile_disable_flag_crlf_and_nbsp_wrapped_false_value_keeps_reconcile_enabled -- --nocapture
cargo test cache_disable_flag_crlf_wrapped_no_value_keeps_cache_enabled_and_uses_existing_cache_entry -- --nocapture
cargo test reconcile_disable_flag_crlf_wrapped_on_disables_reconcile_in_e2e_flow -- --nocapture
cargo test cache_disable_flag_crlf_wrapped_false_value_keeps_cache_enabled_and_uses_existing_cache_entry -- --nocapture
cargo test reconcile_disable_flag_crlf_wrapped_false_value_keeps_reconcile_enabled -- --nocapture
cargo test cache_disable_flag_crlf_wrapped_zero_value_keeps_cache_enabled_and_uses_existing_cache_entry -- --nocapture
cargo test cache_disable_flag_crlf_wrapped_blank_value_keeps_cache_enabled_and_uses_existing_cache_entry -- --nocapture
cargo test reconcile_disable_flag_crlf_wrapped_tabbed_blank_value_keeps_reconcile_enabled -- --nocapture
cargo test cache_disable_flag_unicode_unknown_value_keeps_cache_enabled_and_uses_existing_cache_entry -- --nocapture
cargo test reconcile_disable_flag_unicode_unknown_value_keeps_reconcile_enabled -- --nocapture
cargo test cache_disable_flag_crlf_wrapped_unicode_unknown_value_keeps_cache_enabled_and_uses_existing_cache_entry -- --nocapture
cargo test reconcile_disable_flag_crlf_wrapped_unicode_unknown_value_keeps_reconcile_enabled -- --nocapture
cargo test cache_disable_flag_tab_only_blank_value_keeps_cache_enabled_and_uses_existing_cache_entry -- --nocapture
cargo test reconcile_disable_flag_tab_only_blank_value_keeps_reconcile_enabled -- --nocapture
```

---

## Typical validation sequence after code changes

1. `cargo fmt`
2. targeted `cargo test <...>`
3. `cargo test -- --nocapture`
4. `cargo clippy --all-targets --all-features -- -D warnings`

---

## Notes

- E2E tests live in `tests/e2e.rs`.
- Unit tests live alongside source modules in `src/*`.
- For behavior/edge-case semantics while interpreting failures, use
  [`operational-spec.md`](operational-spec.md).

---

## See also

- [`release-checklist.md`](release-checklist.md)
- [`troubleshooting.md`](troubleshooting.md)
- [`docs index`](README.md)
- [`root README`](../README.md)
