# chopper

`chopper` is a command alias launcher with excessive
flexibility and reasonable performance

It is designed for:

- small, easy-to-maintain alias files
- explicit alias-local args/env config
- optional stderr routing into a journald namespace

If those don't suffice there is an optional Rhai scripting layer
that probably will.

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

## Minimal scripted example

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
ensure = true
max_use = "128M"

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
chopper --bashcomp
chopper --list-aliases
chopper --print-exec <alias>
chopper --print-bashcomp-mode <alias>
chopper --complete <alias> <cword> [--] <words...>
chopper --alias <get|add|set|remove> ...
chopper --tui
```

Alias management examples:

```bash
chopper --alias add demo --exec echo --arg "hello"
chopper --alias set demo --arg "hello-updated" --env APP_ENV=dev
chopper --alias get demo
chopper --alias remove demo --mode clean
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

Truthy values are: `1`, `true`, `yes`, `on` (ASCII case-insensitive, trimmed).
Blank or unknown values (including whitespace/CRLF-wrapped values) keep the
feature enabled.
Common falsey values that also keep features enabled: `0`, `false`, `no`,
`off`.
Truthy matching is ASCII-based; non-ASCII lookalikes (for example `ＴＲＵＥ`)
are unknown values and therefore always true. This is probably a security problem.

---

## Journal Broker Daemon Setup (optional)

Use this when aliases set `[journal] ensure = true`.

Preferred (one-shot, from repo root):

```bash
scripts/install-journal-broker.sh --cleanup-user-install
```

Custom prefix (installs broker binary to `<prefix>/bin` and rewrites service
`ExecStart`/D-Bus `Exec` to match):

```bash
scripts/install-journal-broker.sh --prefix /usr
```

Staging/package install root (installs files under `<destdir>`, skips
`systemctl` actions automatically):

```bash
scripts/install-journal-broker.sh --prefix /usr --destdir /tmp/chopper-pkgroot --no-sudo
```

Manual path:

1) If needed, remove user-local install from `~/.cargo/bin`:

```bash
rm -f ~/.cargo/bin/chopper-journal-broker
cargo uninstall --bin chopper-journal-broker || true
```

2) Build/install both binaries to `/usr/local/bin`:

```bash
cargo build --release --bin chopper --bin chopper-journal-broker
sudo install -m 0755 target/release/chopper /usr/local/bin/chopper
sudo install -m 0755 target/release/chopper-journal-broker /usr/local/bin/chopper-journal-broker
```

3) Install broker service policy files (system-wide):

```bash
sudo cp dist/dbus-1/system.d/com.chopperproject.JournalBroker1.conf \
  /usr/share/dbus-1/system.d/
sudo cp dist/dbus-1/system-services/com.chopperproject.JournalBroker1.service \
  /usr/share/dbus-1/system-services/
sudo cp dist/polkit-1/actions/com.chopperproject.JournalBroker1.policy \
  /usr/share/polkit-1/actions/
sudo cp dist/polkit-1/rules.d/50-chopper-journal-broker.rules \
  /usr/share/polkit-1/rules.d/
sudo cp dist/systemd/chopper-journal-broker.service /etc/systemd/system/
```

4) Reload and start services:

```bash
sudo systemctl daemon-reload
sudo systemctl reload dbus
sudo systemctl enable --now chopper-journal-broker
```

5) Verify broker presence:

```bash
systemctl status chopper-journal-broker
busctl --system introspect \
  com.chopperproject.JournalBroker1 \
  /com/chopperproject/JournalBroker1
```

6) Configure alias journal preflight:

```toml
[journal]
namespace = "ops"
stderr = true
ensure = true
```

With default `user_scope = true`, `chopper` derives
`u<uid>-<sanitized-username>-<sanitized-namespace>` and asks the broker to
prepare it before launching `systemd-cat`.

For full daemon details and troubleshooting, see
[`doc/broker-setup.md`](doc/broker-setup.md).

---

## Detailed documentation

For a full docs map, see [`doc/README.md`](doc/README.md).
code/Contributing guide: [`CONTRIBUTING.md`](CONTRIBUTING.md)
