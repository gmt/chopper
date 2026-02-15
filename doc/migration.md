# chopper migration guide

This page helps migrate existing aliases into the TOML DSL incrementally.

---

## 1) Keep legacy aliases working while migrating

Legacy one-line files are still supported via discovery order, so you can
migrate alias-by-alias without a flag day.

Legacy lookup positions (after TOML paths):

1. `<alias>`
2. `<alias>.conf`
3. `<alias>.rhai`

---

## 2) Convert a one-line alias to TOML

Legacy:

```text
kubectl get pods -A
```

TOML:

```toml
exec = "kubectl"
args = ["get", "pods", "-A"]
```

Recommended location:

```text
~/.config/chopper/aliases/<alias>.toml
```

---

## 3) Move inline env assumptions into `[env]`

Before (implicit via shell profile):

- relies on caller environment state

After (explicit per-alias):

```toml
[env]
KUBECONFIG = "/home/me/.kube/config"
```

Use `env_remove` to strip inherited variables you do not want:

```toml
env_remove = ["AWS_PROFILE"]
```

---

## 4) Add journald routing only where needed

```toml
[journal]
namespace = "ops"
stderr = true
identifier = "my-alias"
```

This keeps logging behavior explicit and scoped to selected aliases.

---

## 5) Introduce Rhai reconcile incrementally

Start with static TOML first, then add `[reconcile]` only for aliases that need
runtime adaptation.

Alias:

```toml
[reconcile]
script = "my.reconcile.rhai"
function = "reconcile"
```

Script:

```rhai
fn reconcile(ctx) {
  let out = #{};
  if ctx.runtime_args.contains("--prod") {
    out["set_env"] = #{ "APP_ENV": "production" };
  }
  out
}
```

---

## 6) Validate behavior while migrating

Useful checks:

- `chopper --print-config-dir`
- `chopper --print-cache-dir`
- `CHOPPER_DISABLE_CACHE=1 chopper <alias> ...` (source-only run)
- `CHOPPER_DISABLE_RECONCILE=1 chopper <alias> ...` (skip reconcile)

---

## 7) Recommended migration order

1. Convert low-risk aliases to TOML first.
2. Make env dependencies explicit (`[env]` / `env_remove`).
3. Enable journald on aliases needing stderr namespace routing.
4. Add reconcile only where static args/env are insufficient.
5. Remove legacy files after each alias is validated.

---

## See also

- [`examples.md`](examples.md)
- [`templates/`](templates)
- [`operational-spec.md`](operational-spec.md)
