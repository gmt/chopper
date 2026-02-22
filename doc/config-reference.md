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

### `user_scope` (optional)

- Type: boolean
- Default: `true`
- Notes:
  - when `true`, `namespace` is treated as a logical user namespace name
  - chopper derives effective namespace:
    - `u<uid>-<sanitized-username>-<sanitized-namespace>`
  - only affects the namespace passed to `systemd-cat --namespace=...`
  - set to `false` for literal namespace passthrough

### `ensure` (optional)

- Type: boolean
- Default: `false`
- Notes:
  - when `true`, chopper calls the `chopper-journal-broker` D-Bus service
    before `systemd-cat`
  - the broker ensures the journald namespace sockets are started and
    drop-in configuration is written
  - D-Bus bus name: `com.chopperproject.JournalBroker1`
  - method: `EnsureNamespace(namespace, options)`
  - broker failure aborts invocation before child process spawn
  - requires `chopper-journal-broker` to be installed as a D-Bus system
    service (see `dist/` for configuration files)

### `max_use` (optional)

- Type: string
- Notes:
  - journald `SystemMaxUse` value for the namespace (e.g. `"256M"`, `"1G"`)
  - passed to broker via D-Bus; broker clamps to hard limit (512M)
  - only effective when `ensure = true`

### `rate_limit_interval_usec` (optional)

- Type: integer (microseconds)
- Notes:
  - journald `RateLimitIntervalSec` for the namespace
  - passed to broker via D-Bus; broker clamps to range [1000, 3600000000]
  - only effective when `ensure = true`

### `rate_limit_burst` (optional)

- Type: integer
- Notes:
  - journald `RateLimitBurst` for the namespace
  - passed to broker via D-Bus; broker clamps to max 10000
  - only effective when `ensure = true`

---

## `[reconcile]` table (optional)

### `script` (legacy, optional)

- Type: string
- Notes:
  - accepted for compatibility only
  - ignored by runtime/TUI wiring
  - emits a warning in diagnostics flows

### `function` (optional)

- Type: string
- Notes:
  - trimmed
  - cannot contain NUL
  - **primary reconcile wiring field**
  - when set, reconcile runs from deterministic shared script `<alias>.rhai`
  - blank/unset disables reconcile

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
  - accepted for compatibility only
  - ignored by runtime/TUI wiring
  - emits a warning in diagnostics flows

### `rhai_function` (optional)

- Type: string
- Notes:
  - completion function name in deterministic shared script `<alias>.rhai`
  - trimmed; blank values treated as unset
  - cannot contain NUL
  - **primary Rhai-completion wiring field**
  - blank/unset disables Rhai completion mode
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
ensure = true
max_use = "128M"

[reconcile]
function = "reconcile"

[bashcomp]
passthrough = true
# or, for Rhai-based completion:
# rhai_function = "complete"
```

---

## See also

- [`cli-reference.md`](cli-reference.md)
- [`operational-spec.md`](operational-spec.md)
- [`docs index`](README.md)
- [`root README`](../README.md)
