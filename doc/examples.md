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

---

## 8) Override config/cache roots

```bash
CHOPPER_CONFIG_DIR=/tmp/chopper-cfg chopper myalias
CHOPPER_CACHE_DIR=/tmp/chopper-cache chopper myalias
```
