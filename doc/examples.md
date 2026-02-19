# chopper examples

Practical patterns you can copy/paste.

For full semantics and edge-case details, see `operational-spec.md`.
If you want ready-to-copy starter files, see `templates/`.

---

## 1) Simple alias

```toml
exec = "echo"
args = ["hello"]
```

Run:

```bash
chopper hello
```

---

## 2) Alias-scoped environment variables

```toml
exec = "env"
args = []

[env]
APP_ENV = "staging"
LOG_LEVEL = "debug"
```

---

## 3) Remove inherited environment variables

```toml
exec = "env"
args = []
env_remove = ["AWS_PROFILE", "GITHUB_TOKEN"]
```

---

## 4) Journald namespace + stderr routing

```toml
exec = "sh"
args = ["-c", "echo ok; echo err >&2"]

[journal]
namespace = "ops"
stderr = true
identifier = "my-alias"
```

## 4b) User-scoped journald namespace with broker preflight

```toml
exec = "sh"
args = ["-c", "echo ok; echo err >&2"]

[journal]
namespace = "ops"
stderr = true
user_scope = true
ensure = true
```

Optional broker override:

```bash
CHOPPER_JOURNAL_BROKER_CMD="/usr/local/bin/chopper-journal-broker --profile user" chopper myalias
```

---

## 5) Reconcile runtime args with Rhai

`aliases/kpods.toml`:

```toml
exec = "kubectl"
args = ["get", "pods"]

[reconcile]
script = "kpods.reconcile.rhai"
```

`aliases/kpods.reconcile.rhai`:

```rhai
fn reconcile(ctx) {
  let out = #{};
  if ctx.runtime_args.contains("--all-ns") {
    out["append_args"] = ["-A"];
  }
  out
}
```

---

## 6) Legacy one-line alias

File: `~/.config/chopper/kpods`

```text
kubectl get pods
```

---

## 7) One-off bypasses for debugging

Bypass cache:

```bash
CHOPPER_DISABLE_CACHE=1 chopper myalias
```

Bypass reconcile:

```bash
CHOPPER_DISABLE_RECONCILE=1 chopper myalias
```

Notes:

- only truthy values disable (`1`, `true`, `yes`, `on`; trimmed,
  ASCII case-insensitive)
- common falsey values (`0`, `false`, `no`, `off`) keep features enabled
- blank/unknown values (including CRLF-wrapped strings) keep features enabled

---

## 8) Override config/cache roots

```bash
CHOPPER_CONFIG_DIR=/tmp/chopper-cfg chopper myalias
CHOPPER_CACHE_DIR=/tmp/chopper-cache chopper myalias
```

---

## 9) Alias admin CLI

```bash
chopper --alias add demo --exec echo --arg hello
chopper --alias set demo --arg hello-updated --env APP_ENV=dev
chopper --alias get demo
chopper --alias remove demo --mode clean
```

Dirty remove (symlink-only):

```bash
chopper --alias remove demo --mode dirty --symlink-path /usr/local/bin/demo
```

---

## 10) TUI workflow

```bash
chopper --tui
```

Then use:

- `a` add alias
- `s` set alias
- `r` remove alias
- `e` edit Rhai script in `(n)vim`

---

## 11) Rhai facade usage in reconcile script

```rhai
fn reconcile(ctx) {
  let out = #{};

  let p = platform_info();
  out["set_env"] = #{ "FAC_OS": p["os"] };

  if fs_exists("runtime-args.txt") {
    out["append_args"] = [fs_read_text("runtime-args.txt").trim()];
  }

  let probe = proc_run("sh", ["-c", "echo probe"], 1000);
  if probe["ok"] {
    out["set_env"]["FAC_PROBE"] = "ok";
  }

  out
}
```
