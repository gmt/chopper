# chopper

`chopper` is an alias launcher with a concrete, per-alias DSL.

It is opinionated:

- alias definitions live in small per-alias files
- TOML is the primary declarative DSL
- legacy one-line aliases still work
- parsed manifests are cached automatically
- optional Rhai hooks can reconcile runtime args/env

---

## Installation / invocation model

`chopper` supports two invocation styles:

1. **Direct**:

```bash
chopper <alias> [args...]
```

You may insert `--` to separate chopper parsing from alias args:

```bash
chopper <alias> -- [args...]
```

2. **Symlinked alias**:

```bash
ln -s /path/to/chopper /usr/local/bin/kpods
kpods [args...]
```

In symlink mode, alias name is inferred from executable name (`kpods` above).
You may also use `kpods -- [args...]` to explicitly separate passthrough args.

---

## Alias config discovery

Config root is `${XDG_CONFIG_HOME:-~/.config}/chopper`.

Lookup order for alias `foo`:

1. `aliases/foo.toml`
2. `foo.toml`
3. `foo`
4. `foo.conf`
5. `foo.rhai`

Files `foo`, `foo.conf`, `foo.rhai` are treated as **legacy one-line command aliases**.

---

## DSL reference (TOML)

```toml
exec = "kubectl"                 # required
args = ["get", "pods", "-A"]     # optional, default []
env_remove = ["AWS_PROFILE"]     # optional, default []

[env]                            # optional map<string,string>
KUBECONFIG = "/home/me/.kube/config"

[journal]                        # optional
namespace = "ops"                # required when [journal] is present
stderr = true                    # optional, default true
identifier = "kpods"             # optional

[reconcile]                      # optional
script = "kpods.reconcile.rhai"  # required
function = "reconcile"           # optional, default "reconcile"
```

### Argument merge order

1. alias `args`
2. runtime args passed at invocation time
3. optional Rhai patch (`replace_args`, then `append_args`)

### Environment merge order

1. process inherits parent environment
2. alias `[env]` is injected
3. alias `env_remove` is removed
4. optional Rhai patch (`set_env`, then `remove_env`)

---

## Journald namespace behavior

When `[journal]` is configured with `stderr = true`, `chopper`:

- launches target command
- captures target stderr
- forwards stderr into `systemd-cat --namespace=<namespace>`
- keeps stdout attached normally

If `systemd-cat` is missing or does not support `--namespace` (systemd < 256),
execution fails with an explicit error.

---

## Optional runtime reconciliation (Rhai)

When `[reconcile]` is set, the script function is called with:

- `runtime_args`
- `runtime_env`
- `alias_args`
- `alias_env`

The function must return a map. Supported keys:

- `append_args: [string]`
- `replace_args: [string]`
- `set_env: #{ string: string }`
- `remove_env: [string]`

Example:

```rhai
fn reconcile(ctx) {
  let out = #{};
  if ctx.runtime_args.contains("--verbose") {
    out["append_args"] = ["-v"];
  }
  out["set_env"] = #{ "RUNTIME_MODE": "true" };
  out
}
```

---

## Caching

Parsed manifests are cached automatically under `${XDG_CACHE_HOME:-~/.cache}/chopper/manifests`.

Cache invalidation is automatic and based on source file path + metadata
(size and mtime). Users do not need to manually manage cache in normal usage.

For extraordinary debugging scenarios, cache can be bypassed per-invocation:

```bash
CHOPPER_DISABLE_CACHE=1 chopper <alias> [args...]
```
