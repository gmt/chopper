# chopper config reference

Field-by-field reference for alias configuration.

For full behavioral semantics and edge cases, see `operational-spec.md`.

---

## Top-level fields

### `exec` (required)

- Type: string
- Meaning: executable or command path
- Notes:
  - cannot be blank after trim
  - cannot contain NUL
  - cannot be `.` or `..`
  - relative paths are resolved from alias file directory

### `args` (optional)

- Type: array of strings
- Default: `[]`
- Notes:
  - entries cannot contain NUL
  - preserved as provided otherwise

### `env_remove` (optional)

- Type: array of strings
- Default: `[]`
- Notes:
  - entries are trimmed
  - blank entries ignored
  - deduplicated (first-seen order)
  - entries cannot contain `=` or NUL

---

## `[env]` table (optional)

- Type: map string -> string
- Notes:
  - keys are trimmed
  - keys must remain unique after trimming
  - keys cannot contain `=` or NUL
  - values cannot contain NUL

---

## `[journal]` table (optional)

### `namespace` (required when table present)

- Type: string
- Notes:
  - trimmed
  - cannot be blank or NUL

### `stderr` (optional)

- Type: boolean
- Default: `true`

### `identifier` (optional)

- Type: string
- Notes:
  - trimmed
  - blank values treated as unset
  - cannot contain NUL

---

## `[reconcile]` table (optional)

### `script` (required when table present)

- Type: string
- Notes:
  - trimmed
  - cannot be blank or NUL
  - relative paths resolved from alias file directory
  - cannot be `.` / `..` or end in invalid path components

### `function` (optional)

- Type: string
- Default: `"reconcile"`
- Notes:
  - trimmed
  - cannot be blank or NUL

---

## `[bashcomp]` table (optional)

Controls bash tab completion behavior for the alias.

### `disabled` (optional)

- Type: boolean
- Default: `false`
- Notes:
  - when `true`, completion is entirely suppressed
  - the completion function returns immediately with no side effects

### `passthrough` (optional)

- Type: boolean
- Default: `false`
- Notes:
  - when `true`, completion delegates directly to the underlying command
  - Rhai argument transformation is not applied

### `script` (optional)

- Type: string
- Notes:
  - trimmed
  - blank values treated as unset
  - cannot contain NUL
  - relative paths resolved from alias file directory
  - cannot be `.` / `..` or end in invalid path components
  - must define `_chopper_bashcomp_<alias>()` function

### `rhai_script` (optional)

- Type: string
- Notes:
  - path to a Rhai script providing completion logic
  - same path resolution and validation rules as `script`
  - the script must define a function (see `rhai_function`)
  - enables `rhai` mode for `--print-bashcomp-mode`

### `rhai_function` (optional)

- Type: string
- Default: `"complete"`
- Notes:
  - function name within the `rhai_script`
  - requires `rhai_script` to be set
  - trimmed; blank values treated as unset
  - cannot contain NUL
  - receives context map, must return array of candidate strings

---

## Minimal valid example

```toml
exec = "echo"
args = ["hello"]
```

## Full-featured example

```toml
exec = "kubectl"
args = ["get", "pods"]
env_remove = ["AWS_PROFILE"]

[env]
KUBECONFIG = "/home/me/.kube/config"

[journal]
namespace = "ops"
stderr = true
identifier = "kpods"

[reconcile]
script = "kpods.reconcile.rhai"
function = "reconcile"

[bashcomp]
passthrough = true
# or, for Rhai-based completion:
# rhai_script = "completions/kpods.rhai"
# rhai_function = "complete"
```

---

## See also

- [`cli-reference.md`](cli-reference.md)
- [`operational-spec.md`](operational-spec.md)
- [`docs index`](README.md)
- [`root README`](../README.md)
