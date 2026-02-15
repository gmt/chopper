# chopper

`chopper` is a lightweight command alias launcher with a concrete per-alias DSL.

It is designed for:

- small, easy-to-maintain alias files
- explicit alias-local args/env config
- optional stderr routing into a journald namespace
- optional runtime reconciliation via Rhai
- automatic manifest caching (no manual cache management needed)

---

## Section map

- [Install / run](#install--run)
- [Quickstart](#quickstart)
- [Minimal DSL example](#minimal-dsl-example)
- [Invocation styles](#invocation-styles)
- [Useful built-ins (direct mode)](#useful-built-ins-direct-mode)
- [Override directories](#override-directories)
- [Optional feature toggles (per invocation)](#optional-feature-toggles-per-invocation)
- [Detailed documentation](#detailed-documentation)

---

## Install / run

Build and run directly from source:

```bash
cargo run -- <alias> [args...]
```

Install to your Cargo bin dir:

```bash
cargo install --path .
```

Then invoke:

```bash
chopper <alias> [args...]
```

---

## Quickstart

### 1) Create an alias

Create a file at:

```text
~/.config/chopper/aliases/hello.toml
```

with:

```toml
exec = "echo"
args = ["hello from chopper"]
```

### 2) Run it

```bash
chopper hello
```

You can pass runtime args:

```bash
chopper hello world
```

---

## Minimal DSL example

```toml
exec = "kubectl"
args = ["get", "pods", "-A"]
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
```

---

## Invocation styles

### Direct

```bash
chopper <alias> [args...]
```

### Symlinked alias

```bash
ln -s /path/to/chopper /usr/local/bin/kpods
kpods [args...]
```

In symlink mode, the executable basename (`kpods`) is the alias name.

---

## Useful built-ins (direct mode)

```bash
chopper --help
chopper --version
chopper --print-config-dir
chopper --print-cache-dir
```

---

## Override directories

```bash
CHOPPER_CONFIG_DIR=/path/to/config-root chopper <alias> [args...]
CHOPPER_CACHE_DIR=/path/to/cache-root chopper <alias> [args...]
```

Whitespace wrappers are trimmed; path shape is otherwise preserved.

---

## Optional feature toggles (per invocation)

```bash
CHOPPER_DISABLE_CACHE=1 chopper <alias> [args...]
CHOPPER_DISABLE_RECONCILE=1 chopper <alias> [args...]
```

Truthy values are: `1`, `true`, `yes`, `on` (case-insensitive, trimmed).

---

## Detailed documentation

Use [`doc/README.md`](doc/README.md) as the documentation entry point.

Fast routing:

- command lookup: [`doc/quick-reference.md`](doc/quick-reference.md)
- copy/paste workflows: [`doc/examples.md`](doc/examples.md)
- migration from legacy aliases: [`doc/migration.md`](doc/migration.md)
- terminology lookup: [`doc/glossary.md`](doc/glossary.md)
- troubleshooting: [`doc/troubleshooting.md`](doc/troubleshooting.md)
- architecture overview: [`doc/architecture.md`](doc/architecture.md)
- complete operational semantics: [`doc/operational-spec.md`](doc/operational-spec.md)
