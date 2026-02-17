# Rhai facade API reference

High-level APIs available to Rhai reconcile/completion scripts.

These APIs are intent-level abstractions, not thin wrappers over platform-specific
types.

---

## Profile availability

- **Reconcile profile** (`[reconcile]` scripts):
  - platform
  - fs read/write
  - process
  - web fetch
  - soap
- **Completion profile** (`bashcomp.rhai_script` scripts):
  - platform
  - fs read-only (`fs_exists`, `fs_stat`, `fs_list`, `fs_read_text`)
  - process/web/soap are intentionally not exposed in completion hot-path.

---

## Platform facade

- `platform_info() -> map`
- `platform_is_windows() -> bool`
- `platform_is_unix() -> bool`
- `executable_intent(path) -> map`
- `can_execute_without_confirmation(path) -> bool`
- `can_execute_with_confirmation(path) -> bool`

`executable_intent(path)` returns:

- `exists`
- `is_file`
- `is_dir`
- `can_execute_without_confirmation`
- `can_execute_with_confirmation`
- `requires_user_confirmation`

---

## File facade (cap-std based)

- `fs_exists(path) -> bool`
- `fs_stat(path) -> map`
- `fs_list(path) -> [string]`
- `fs_read_text(path) -> string`
- `fs_write_text(path, text) -> map` *(reconcile only)*
- `fs_mkdir(path, recursive_bool) -> map` *(reconcile only)*
- `fs_remove(path, recursive_bool) -> map` *(reconcile only)*

---

## Process facade (duct based, reconcile only)

- `proc_run(exec, args_array, timeout_ms) -> map`
- `proc_run_with(exec, args_array, env_map, cwd, timeout_ms) -> map`

Response map includes:

- `ok`
- `timed_out`
- `status`
- `stdout`
- `stderr`

---

## Web facade (curl-ish, reconcile only)

- `web_fetch(url) -> map` (GET, 10s timeout)
- `web_fetch_with(method, url, headers_map, body, timeout_ms) -> map`

Response map includes:

- `ok`
- `status`
- `method`
- `url`
- `headers`
- `body`
- `error` (when request fails)

---

## SOAP facade (reconcile only)

- `soap_envelope(body_xml) -> string`
- `soap_call(url, action, body_xml, timeout_ms) -> map`

Response map includes:

- `ok`
- `status`
- `body`
- `fault`
- `fault_text`
- `error` (when request fails)

---

## Example

```rhai
fn reconcile(ctx) {
  let out = #{};

  let p = platform_info();
  out["set_env"] = #{ "RHAI_PLATFORM_OS": p["os"] };

  if fs_exists("config/extra.args") {
    let extra = fs_read_text("config/extra.args");
    out["append_args"] = [extra.trim()];
  }

  let probe = proc_run("sh", ["-c", "echo probe-ok"], 1000);
  if probe["ok"] {
    out["set_env"]["RHAI_PROBE"] = "ok";
  }

  out
}
```
