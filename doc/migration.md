# chopper migration guide

Legacy one-line alias configs are no longer supported. Alias discovery is now
TOML-only:

1. `aliases/<alias>.toml`
2. `<alias>.toml`

If you still have historical one-line aliases in backups or old branches,
convert them to TOML before use.

---

## Convert one-line alias to TOML

Before:

```text
kubectl get pods -A
```

After:

```toml
exec = "kubectl"
args = ["get", "pods", "-A"]
```

Recommended location:

```text
~/.config/chopper/aliases/<alias>.toml
```

---

## Recommended rollout

1. Convert command/args into TOML `exec` + `args`.
2. Move environment assumptions into `[env]` / `env_remove`.
3. Add `[journal]` and `[reconcile]` only where needed.
4. Validate with:
   - `chopper --print-config-dir`
   - `chopper --print-cache-dir`
   - `CHOPPER_DISABLE_CACHE=1 chopper <alias> ...`
   - `CHOPPER_DISABLE_RECONCILE=1 chopper <alias> ...`

---

## See also

- [`examples.md`](examples.md)
- [`templates/`](templates)
- [`operational-spec.md`](operational-spec.md)
