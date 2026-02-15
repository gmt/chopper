# chopper troubleshooting

This page helps with common operational issues.

For complete behavior semantics, see `operational-spec.md`.

---

## 1) Alias not found

Symptoms:

- `alias '<name>' not found`

Checks:

1. Confirm the alias file location:
   - `~/.config/chopper/aliases/<alias>.toml` (preferred), or
   - `~/.config/chopper/<alias>.toml`
2. If using an override:
   - check `CHOPPER_CONFIG_DIR`
   - run `chopper --print-config-dir` to confirm effective root
3. Verify alias name is a logical identifier (not a filesystem path token).

---

## 2) Built-in flags behaving like alias args

Symptoms:

- `--help` or `--version` is passed through unexpectedly.

Explanation:

- Built-ins are only recognized in **direct mode** (`chopper ...`).
- In **symlink mode**, those flags are treated as passthrough runtime args.

---

## 3) Journal namespace errors

Symptoms:

- journal startup failure / namespace-related error

Checks:

1. Ensure `systemd-cat` exists on PATH.
2. Ensure systemd supports `--namespace` (systemd v256+).
3. Validate `[journal]` fields are non-blank after trimming.

---

## 4) Reconcile script not applying

Symptoms:

- expected Rhai patch output not reflected

Checks:

1. Verify `[reconcile]` block exists on the alias.
2. Confirm `script` path is valid (relative paths are resolved from alias file dir).
3. Confirm `function` name exists in the script.
4. Check if reconcile is intentionally disabled:
   - `CHOPPER_DISABLE_RECONCILE=1|true|yes|on`

---

## 5) Cache appears stale

Symptoms:

- output seems to reflect old alias data

Checks:

1. Try a one-off bypass:
   - `CHOPPER_DISABLE_CACHE=1 chopper <alias> ...`
2. Confirm you edited the source file that is actually resolved by lookup order.
3. Re-run; cache entries are invalidated by source metadata and self-heal when corrupted.

---

## 6) Environment overrides not taking effect

Symptoms:

- `CHOPPER_CONFIG_DIR` / `CHOPPER_CACHE_DIR` seemingly ignored

Checks:

1. Validate effective roots:
   - `chopper --print-config-dir`
   - `chopper --print-cache-dir`
2. Blank values are ignored.
3. Leading/trailing wrapper whitespace is trimmed; inner path shape is preserved.

---

## 7) Invalid string / validation errors

Common causes:

- NUL bytes in args/env/journal/reconcile strings
- `=` in env keys or remove lists
- blank-after-trim values in required fields
- invalid `exec` or `reconcile.script` path forms (`.`, `..`, trailing separators)

Use `operational-spec.md` for full validation rules and precedence semantics.
